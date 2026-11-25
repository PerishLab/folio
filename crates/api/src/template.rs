use crate::{App, Failure, Strict, document, failure};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    middleware,
    routing::{MethodRouter, get, post},
};
use folio::{Create, Detail, Edition, Identity, Publish, Status, Templates};
use std::sync::Arc;

pub(super) fn collection() -> MethodRouter<Arc<App>> {
    get(|State(app): State<Arc<App>>, Strict(identity): Strict| async move {
        app.templates(identity).await
    })
    .post(
        |State(app): State<Arc<App>>, Strict(identity): Strict, Json(input): Json<Create>| async move {
            app.register(identity, input).await
        },
    )
}

pub(super) fn item() -> MethodRouter<Arc<App>> {
    get(
        |State(app): State<Arc<App>>, Path(name): Path<String>, Strict(identity): Strict| async move {
            app.template(identity, name).await
        },
    )
}

pub(super) fn editions() -> MethodRouter<Arc<App>> {
    post(
        |State(app): State<Arc<App>>,
         Path(name): Path<String>,
         Strict(identity): Strict,
         Json(input): Json<Publish>| async move { app.release(identity, name, input).await },
    )
}

pub(super) fn edition() -> MethodRouter<Arc<App>> {
    get(
        |State(app): State<Arc<App>>,
         Path((name, number)): Path<(String, i64)>,
         Strict(identity): Strict| async move { app.edition(identity, name, number).await },
    )
}

pub(super) fn status() -> MethodRouter<Arc<App>> {
    get(
        |State(app): State<Arc<App>>, Path(name): Path<String>, Strict(identity): Strict| async move {
            app.report(identity, name).await
        },
    )
    .layer(middleware::map_response(document::cache))
}

impl App {
    async fn templates(&self, identity: Identity) -> Result<Json<Templates>, Failure> {
        self.runtime
            .templates(&identity)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn register(
        &self,
        identity: Identity,
        input: Create,
    ) -> Result<(StatusCode, Json<Edition>), Failure> {
        let created = self
            .runtime
            .register(&identity, &input)
            .await
            .map_err(failure)?;
        Ok((StatusCode::CREATED, Json(created)))
    }

    async fn template(&self, identity: Identity, name: String) -> Result<Json<Detail>, Failure> {
        self.runtime
            .template(&identity, &name)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn release(
        &self,
        identity: Identity,
        name: String,
        input: Publish,
    ) -> Result<(StatusCode, Json<Edition>), Failure> {
        let created = self
            .runtime
            .release(&identity, &name, &input)
            .await
            .map_err(failure)?;
        Ok((StatusCode::CREATED, Json(created)))
    }

    async fn edition(
        &self,
        identity: Identity,
        name: String,
        number: i64,
    ) -> Result<Json<Edition>, Failure> {
        self.runtime
            .edition(&identity, &name, number)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn report(&self, identity: Identity, name: String) -> Result<Json<Status>, Failure> {
        self.runtime
            .status(&identity, &name)
            .await
            .map(Json)
            .map_err(failure)
    }
}
