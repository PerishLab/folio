use crate::{App, Failure, Selection, failure};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, header},
    middleware,
    response::Response,
    routing::{MethodRouter, get},
};
use folio::{Snapshot, Write};
use std::sync::Arc;

pub(super) fn route() -> MethodRouter<Arc<App>> {
    get(read)
        .patch(write)
        .layer(middleware::map_response(cache))
}

async fn read(
    State(app): State<Arc<App>>,
    Query(selection): Query<Selection>,
    headers: HeaderMap,
) -> Result<Json<Snapshot>, Failure> {
    let identity = app.identity(&headers)?;
    let device = selection.device()?;
    app.runtime
        .snapshot(&identity, selection.app.as_deref(), device)
        .await
        .map(Json)
        .map_err(failure)
}

async fn write(
    State(app): State<Arc<App>>,
    Query(selection): Query<Selection>,
    headers: HeaderMap,
    Json(write): Json<Write>,
) -> Result<Json<Snapshot>, Failure> {
    let identity = app.identity(&headers)?;
    let device = selection.device()?;
    app.runtime
        .confirm(&identity, selection.app.as_deref(), &write, device)
        .await
        .map(Json)
        .map_err(failure)
}

async fn cache(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
