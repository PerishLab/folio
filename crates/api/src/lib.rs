#![forbid(unsafe_code)]

mod document;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use folio::{Auth, Device, Error, Runtime, Write};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

struct App {
    runtime: Runtime,
    auth: Auth,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    app: Option<String>,
    tone: Option<String>,
}

impl Selection {
    fn device(&self) -> Result<Device, Failure> {
        Device::new(self.tone.as_deref()).map_err(failure)
    }
}

type Failure = (StatusCode, Json<Value>);

pub fn router(runtime: Runtime, auth: Auth) -> Router {
    Router::new()
        .route(
            "/health",
            get(|State(app): State<Arc<App>>| async move { app.health().await }),
        )
        .route(
            "/v1/me/settings",
            get(
                |State(app): State<Arc<App>>,
                 Query(selection): Query<Selection>,
                 headers: HeaderMap| async move { app.read(selection, headers).await },
            )
            .patch(
                |State(app): State<Arc<App>>,
                 Query(selection): Query<Selection>,
                 headers: HeaderMap,
                 Json(write): Json<Write>| async move {
                    app.write(selection, headers, write).await
                },
            ),
        )
        .route(
            "/v1/me/settings/{name}",
            get(
                |State(app): State<Arc<App>>,
                 Path(name): Path<String>,
                 Query(selection): Query<Selection>,
                 headers: HeaderMap| async move {
                    app.one(name, selection, headers).await
                },
            ),
        )
        .route("/v1/me/documents/preferences", document::route())
        .with_state(Arc::new(App { runtime, auth }))
}

impl App {
    async fn health(&self) -> Result<&'static str, Failure> {
        self.runtime.health().await.map_err(failure)?;
        Ok("ok")
    }

    fn identity(&self, headers: &HeaderMap) -> Result<folio::Identity, Failure> {
        let held = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(|| failure(Error::Unauthorized))?;
        self.auth.verify(held).map_err(failure)
    }

    async fn read(
        &self,
        selection: Selection,
        headers: HeaderMap,
    ) -> Result<Json<folio::State>, Failure> {
        let identity = self.identity(&headers)?;
        let device = selection.device()?;
        self.runtime
            .render(&identity, selection.app.as_deref(), device)
            .await
            .map(Json)
            .map_err(failure)
    }

    async fn one(
        &self,
        name: String,
        selection: Selection,
        headers: HeaderMap,
    ) -> Result<Json<Value>, Failure> {
        let identity = self.identity(&headers)?;
        let device = selection.device()?;
        let state = self
            .runtime
            .render(&identity, selection.app.as_deref(), device)
            .await
            .map_err(failure)?;
        let item = state
            .items
            .iter()
            .find(|item| item.name == name)
            .ok_or_else(|| failure(Error::Missing))?;
        Ok(Json(
            json!({"revision": state.revision, "app": state.app, "item": item}),
        ))
    }

    async fn write(
        &self,
        selection: Selection,
        headers: HeaderMap,
        write: Write,
    ) -> Result<Json<folio::State>, Failure> {
        let identity = self.identity(&headers)?;
        let device = selection.device()?;
        self.runtime
            .submit(&identity, selection.app.as_deref(), &write, device)
            .await
            .map(Json)
            .map_err(failure)
    }
}

fn failure(error: Error) -> Failure {
    let status = match error.code() {
        "unauthorized" => StatusCode::UNAUTHORIZED,
        "denied" => StatusCode::FORBIDDEN,
        "invalid" => StatusCode::UNPROCESSABLE_ENTITY,
        "missing" => StatusCode::NOT_FOUND,
        "conflict" => StatusCode::CONFLICT,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status, Json(json!({"error": error.code()})))
}
