use crate::{App, Caller, Failure, Strict, document, failure};
use axum::{
    Json,
    body::Bytes,
    extract::{FromRequestParts, Path, Query, State},
    http::{StatusCode, request::Parts},
    middleware,
    routing::{MethodRouter, get, post},
};
use folio::{Amend, Draft, Entry, Error, Identity, Listing, Migrate, Tags};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Delete {
    revision: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revise {
    revision: String,
}

struct Target {
    id: String,
    revision: Option<String>,
}

impl FromRequestParts<Arc<App>> for Target {
    type Rejection = Failure;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<App>,
    ) -> Result<Self, Self::Rejection> {
        let Path(id) = Path::<String>::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid"}))))?;
        let Query(delete) = Query::<Delete>::from_request_parts(parts, state)
            .await
            .map_err(|_| (StatusCode::BAD_REQUEST, Json(json!({"error":"invalid"}))))?;
        Ok(Self {
            id,
            revision: delete.revision,
        })
    }
}

pub(super) fn tags() -> MethodRouter<Arc<App>> {
    get(|State(app): State<Arc<App>>, Strict(identity): Strict| async move {
        app.tags(identity).await
    })
    .patch(
        |State(app): State<Arc<App>>, Strict(identity): Strict, Json(input): Json<Amend>| async move {
            app.retag(identity, input).await
        },
    )
    .layer(middleware::map_response(document::cache))
}

pub(super) fn collection() -> MethodRouter<Arc<App>> {
    get(|State(app): State<Arc<App>>, Strict(identity): Strict| async move {
        app.entries(identity).await
    })
    .post(
        |State(app): State<Arc<App>>, Strict(identity): Strict, Json(draft): Json<Draft>| async move {
            app.insert(identity, draft).await
        },
    )
    .layer(middleware::map_response(document::cache))
}

pub(super) fn item() -> MethodRouter<Arc<App>> {
    get(
        |State(app): State<Arc<App>>, Path(id): Path<String>, Strict(identity): Strict| async move {
            app.entry(identity, id).await
        },
    )
    .patch(
        |State(app): State<Arc<App>>,
         Path(id): Path<String>,
         Strict(identity): Strict,
         Json(input): Json<Amend>| async move { app.amend(identity, id, input).await },
    )
    .delete(
        |State(app): State<Arc<App>>, target: Target, Caller(identity): Caller, body: Bytes| async move {
            app.erase(identity, target, body).await
        },
    )
    .layer(middleware::map_response(document::cache))
}

pub(super) fn migrate() -> MethodRouter<Arc<App>> {
    post(
        |State(app): State<Arc<App>>,
         Path(id): Path<String>,
         Strict(identity): Strict,
         Json(input): Json<Migrate>| async move { app.upgrade(identity, id, input).await },
    )
    .layer(middleware::map_response(document::cache))
}

impl App {
    async fn tags(&self, identity: Identity) -> Result<Json<Tags>, Failure> {
        self.runtime
            .tags(&identity)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn retag(&self, identity: Identity, input: Amend) -> Result<Json<Tags>, Failure> {
        self.runtime
            .tag(&identity, &input)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn entries(&self, identity: Identity) -> Result<Json<Listing>, Failure> {
        self.runtime
            .entries(&identity)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn insert(
        &self,
        identity: Identity,
        draft: Draft,
    ) -> Result<(StatusCode, Json<Entry>), Failure> {
        let created = self
            .runtime
            .insert(&identity, &draft)
            .await
            .map_err(failure)?;
        Ok((StatusCode::CREATED, Json(created)))
    }

    async fn entry(&self, identity: Identity, id: String) -> Result<Json<Entry>, Failure> {
        self.runtime
            .entry(&identity, &id)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn amend(
        &self,
        identity: Identity,
        id: String,
        input: Amend,
    ) -> Result<Json<Entry>, Failure> {
        self.runtime
            .amend(&identity, &id, &input)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn upgrade(
        &self,
        identity: Identity,
        id: String,
        input: Migrate,
    ) -> Result<Json<Entry>, Failure> {
        self.runtime
            .migrate(&identity, &id, &input)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn erase(
        &self,
        identity: Identity,
        target: Target,
        body: Bytes,
    ) -> Result<StatusCode, Failure> {
        let revision = revision(target.revision, &body)?;
        self.runtime
            .remove(&identity, &target.id, &revision)
            .await
            .map_err(failure)?;
        Ok(StatusCode::NO_CONTENT)
    }
}

fn revision(query: Option<String>, body: &[u8]) -> Result<String, Failure> {
    let trimmed = body.iter().any(|byte| !byte.is_ascii_whitespace());
    let value = match (query, trimmed) {
        (Some(held), false) => held,
        (Some(held), true) => {
            let parsed: Revise = serde_json::from_slice(body)
                .map_err(|_| failure(Error::Invalid("revision".into())))?;
            if parsed.revision != held {
                return Err(failure(Error::Invalid("revision".into())));
            }
            held
        }
        (None, true) => {
            let parsed: Revise = serde_json::from_slice(body)
                .map_err(|_| failure(Error::Invalid("revision".into())))?;
            parsed.revision
        }
        (None, false) => return Err(failure(Error::Invalid("revision".into()))),
    };
    if value.is_empty() {
        return Err(failure(Error::Invalid("revision".into())));
    }
    Ok(value)
}
