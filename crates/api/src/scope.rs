use crate::{App, Empty, Failure, document, failure};
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
    middleware,
    routing::{MethodRouter, get},
};
use folio::{Device, Scopes, Workbench, Write};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fact {
    tone: Option<String>,
}

pub(super) fn scopes() -> MethodRouter<Arc<App>> {
    get(list)
}

pub(super) fn workbench() -> MethodRouter<Arc<App>> {
    get(read)
        .patch(write)
        .layer(middleware::map_response(document::cache))
}

async fn list(
    State(app): State<Arc<App>>,
    Query(_): Query<Empty>,
    headers: HeaderMap,
) -> Result<Json<Scopes>, Failure> {
    let _ = app.identity(&headers)?;
    Ok(Json(app.runtime.scopes()))
}

async fn read(
    State(app): State<Arc<App>>,
    Query(fact): Query<Fact>,
    headers: HeaderMap,
) -> Result<Json<Workbench>, Failure> {
    let identity = app.identity(&headers)?;
    let device = Device::new(fact.tone.as_deref()).map_err(failure)?;
    app.runtime
        .workbench(&identity, device)
        .await
        .map(Json)
        .map_err(failure)
}

async fn write(
    State(app): State<Arc<App>>,
    Query(fact): Query<Fact>,
    headers: HeaderMap,
    Json(write): Json<Write>,
) -> Result<Json<Workbench>, Failure> {
    let identity = app.identity(&headers)?;
    let device = Device::new(fact.tone.as_deref()).map_err(failure)?;
    app.runtime
        .revise(&identity, &write, device)
        .await
        .map(Json)
        .map_err(failure)
}
