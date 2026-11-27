#[path = "../support/mod.rs"]
mod support;

mod gates;

use api::config::Settings;
use axum::{
    Router,
    body::Body,
    extract::{Query, State},
    http::{HeaderMap, Request, StatusCode},
    routing::post,
};
use flate2::{Compression, write::GzEncoder};
use folio::{Catalog, Runtime};
use locus_relay::{Config, Relay};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use support::Keys;
use tokio::net::TcpListener;
use tower::ServiceExt;

pub(crate) const PRODUCER: &str = "folio-web";
pub(crate) const SECRET: &str = "writer-secret-token-value";

pub(crate) fn atom(id: &str, producer: &str, payload: Value) -> String {
    json!({
        "at": 1700000000000000000_u64,
        "id": id,
        "producer": producer,
        "context": { "locus.schema": "locus.log", "level": "info" },
        "choices": [{ "role": "trace", "key": "abc", "origin": "generated" }],
        "payload": payload
    })
    .to_string()
}

pub(crate) fn gzip(data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}

pub(crate) async fn setup(root: &std::path::Path, config: Config) -> (Router, Relay) {
    let keys = Keys::new();
    let runtime = Runtime::create(root, Catalog::themes(), support::ISSUER.into())
        .await
        .unwrap();
    let relay = Relay::open(config).unwrap();
    (api::mount(runtime, keys.auth(), Some(relay.clone())), relay)
}

pub(crate) fn spec(root: &std::path::Path, endpoint: &str) -> Config {
    Config::new(endpoint, SECRET, PRODUCER, root.join("relay"))
        .origins(vec!["https://folio.example".into()])
        .rpm(120)
        .burst(30)
        .forwarded(false)
}

pub(crate) fn postreq(uri: &str, body: Body) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("sec-fetch-site", "same-origin")
        .header("origin", "https://folio.example")
        .header("content-type", "application/x-ndjson")
        .body(body)
        .unwrap()
}

pub(crate) async fn status(app: &Router, req: Request<Body>) -> StatusCode {
    app.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn disabled() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    std::fs::write(
        root.path().join("folio.toml"),
        format!(
            "issuer = {:?}\naudience = {:?}\n",
            support::ISSUER,
            support::AUDIENCE
        ),
    )
    .unwrap();
    let settings = Settings::from(root.path(), &|_| None).unwrap();
    assert!(settings.relay(root.path()).unwrap().is_none());
    let runtime = Runtime::create(
        &root.path().join("data"),
        Catalog::themes(),
        support::ISSUER.into(),
    )
    .await
    .unwrap();
    let app = api::mount(runtime, keys.auth(), None);
    let body = atom("1", PRODUCER, json!({"message": "hi"}));
    assert_eq!(
        status(&app, postreq("/api/observe", Body::from(body.clone()))).await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        status(&app, postreq("/observe", Body::from(body))).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn config() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("folio.toml"),
        format!(
            "issuer = {:?}\naudience = {:?}\n[locus]\nendpoint = \"https://api.locus.example\"\n",
            support::ISSUER,
            support::AUDIENCE
        ),
    )
    .unwrap();
    let missing = Settings::from(root.path(), &|_| None).unwrap();
    assert!(missing.relay(root.path()).is_err());
    let primary = Settings::from(root.path(), &|name| match name {
        "FOLIO_LOCUS_KEY" => Some("folio-secret-value".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(primary.locus.key, "folio-secret-value");
    let debug = format!("{:?}", primary.locus);
    assert!(debug.contains("[redacted]") && !debug.contains("folio-secret-value"));
    assert!(primary.relay(root.path()).unwrap().is_some());
    let legacy = Settings::from(root.path(), &|name| match name {
        "API_LOCUS_KEY" => Some("legacy-secret-value".into()),
        _ => None,
    })
    .unwrap();
    assert!(legacy.locus.key.is_empty());
    assert!(legacy.relay(root.path()).is_err());
    let both = Settings::from(root.path(), &|name| match name {
        "FOLIO_LOCUS_KEY" => Some("folio-wins".into()),
        "API_LOCUS_KEY" => Some("legacy-loses".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(both.locus.key, "folio-wins");
    std::fs::write(root.path().join("writer.key"), format!("{SECRET}\n")).unwrap();
    std::fs::write(
        root.path().join("folio.toml"),
        format!(
            "issuer = {:?}\naudience = {:?}\n[locus]\nendpoint = \"https://api.locus.example\"\nkeyfile = \"writer.key\"\n",
            support::ISSUER,
            support::AUDIENCE
        ),
    )
    .unwrap();
    let fromfile = Settings::from(root.path(), &|_| None).unwrap();
    assert!(fromfile.relay(root.path()).unwrap().is_some());
}

type Log = Arc<Mutex<Vec<(String, String, String)>>>;

#[tokio::test]
async fn delivery() {
    let captured: Log = Arc::new(Mutex::new(Vec::new()));
    let state = captured.clone();
    let mock = Router::new()
        .route(
            "/api/v1/atoms",
            post(
                |State(log): State<Log>,
                 headers: HeaderMap,
                 Query(params): Query<HashMap<String, String>>,
                 body: String| async move {
                    let auth = headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("")
                        .to_owned();
                    let producer = params.get("producer").cloned().unwrap_or_default();
                    log.lock().unwrap().push((auth, producer, body));
                    StatusCode::NO_CONTENT
                },
            ),
        )
        .with_state(state);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, mock).await;
    });
    let root = tempfile::tempdir().unwrap();
    let (app, relay) = setup(
        &root.path().join("data"),
        spec(root.path(), &format!("http://{addr}")),
    )
    .await;
    let line = atom("evt-1", PRODUCER, json!({"message": "hello"}));
    assert_eq!(
        status(
            &app,
            postreq("/api/observe", Body::from(format!("{line}\n")))
        )
        .await,
        StatusCode::NO_CONTENT
    );
    tokio::task::spawn_blocking(move || relay.settle())
        .await
        .unwrap()
        .unwrap();
    server.abort();
    let records = captured.lock().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].0, format!("Bearer {SECRET}"));
    assert_eq!(records[0].1, PRODUCER);
    assert!(records[0].2.contains("\"evt-1\""));
}
