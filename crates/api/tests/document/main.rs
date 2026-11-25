#[path = "../support/mod.rs"]
mod support;

mod entries;
mod migrate;
mod tags;
mod templates;
mod workbench;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use folio::{Catalog, Decision, Definition};
use keel_document::Shape;
use serde_json::{Value, json};
use support::*;
use tower::ServiceExt;

const DOCUMENT: &str = "/v1/me/documents/preferences?app=ensign&tone=dark";

pub(crate) async fn stored(router: &Router, ask: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(ask).await.unwrap();
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let parsed =
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes)));
    (status, parsed)
}

async fn exchange(router: &Router, method: &str, token: &str, body: Value) -> (StatusCode, Value) {
    stored(router, request(method, DOCUMENT, token, body)).await
}

#[tokio::test]
async fn confirmed() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let (status, read) = exchange(&router, "GET", &token, Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        read["document"]["shared"]["presentation"]["basis"],
        "device"
    );
    assert_eq!(read["document"]["app"]["app"], "ensign");
    let intent = json!({"revision":read["revision"], "changes":[
        set(shared(), "theme.tone", json!("light")),
        set(app(), "theme.tone", json!("dark")),
    ]});
    let (status, saved) = exchange(&router, "PATCH", &token, intent.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(saved["revision"], read["revision"]);
    let shared = &saved["document"]["shared"];
    let app = &saved["document"]["app"];
    assert_eq!(shared["presentation"]["tone"], "light");
    assert_eq!(app["presentation"]["tone"], "dark");
    assert_eq!(shared["items"]["theme.tone"]["source"], "shared");
    assert_eq!(app["items"]["theme.tone"]["source"], "app");
    assert_eq!(app["items"]["theme.tone"]["shared"]["value"], "light");
    assert_eq!(app["items"]["theme.tone"]["app"]["value"], "dark");
    assert_eq!(app["items"]["theme.tone"]["editable"], true);
    assert_eq!(
        exchange(&router, "PATCH", &token, intent).await,
        (StatusCode::CONFLICT, json!({"error":"conflict"}))
    );
    assert_eq!(exchange(&router, "GET", &token, Value::Null).await.1, saved);
    let path = "/v1/me/documents/preferences";
    let (_, shared) = call(&router, request("GET", path, &token, Value::Null)).await;
    assert_eq!(shared["revision"], saved["revision"]);
    assert!(shared["document"]["app"].is_null());
}

#[tokio::test]
async fn private() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let alice = keys.token("alice");
    let bob = keys.token("bob");
    let (_, first) = exchange(&router, "GET", &alice, Value::Null).await;
    let intent = json!({"revision":first["revision"], "changes":[set(shared(), "theme.tone", json!("dark"))]});
    let (_, saved) = exchange(&router, "PATCH", &alice, intent).await;
    let (_, other) = exchange(&router, "GET", &bob, Value::Null).await;
    assert_eq!(
        other["document"]["shared"]["items"]["theme.tone"]["shared"]["state"],
        "missing"
    );
    let intent = json!({"revision":saved["revision"], "changes":[set(shared(), "theme.tone", json!("light"))]});
    assert_eq!(
        exchange(&router, "PATCH", &bob, intent.clone()).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        exchange(&router, "PATCH", "", intent.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    let mut claims = keys.claims("alice");
    claims.exp = claims.iat + 1;
    let expired = keys.signer.sign(&claims).unwrap();
    assert_eq!(
        exchange(&router, "GET", expired.expose(), Value::Null)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        exchange(&router, "PATCH", expired.expose(), intent).await.0,
        StatusCode::UNAUTHORIZED
    );
    let path = format!("{DOCUMENT}&subject=alice");
    assert_eq!(
        call(&router, request("GET", &path, &bob, Value::Null))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(exchange(&router, "GET", &alice, Value::Null).await.1, saved);
    assert_eq!(exchange(&router, "GET", &bob, Value::Null).await.1, other);
}

fn catalog() -> Catalog {
    let definition = Definition {
        name: "label".into(),
        shape: Shape::Nullable(Box::new(Shape::Text)),
        default: json!("first"),
        options: vec![],
    };
    Catalog::new(vec![definition], vec!["ensign".into()])
        .unwrap()
        .policy(|context, _| {
            let mut decision = Decision::default();
            if context.app.is_none() && context.values["apps"]["ensign"]["label"] == "broken" {
                decision.options = Some(vec![]);
            }
            decision
        })
}

#[tokio::test]
async fn rollback() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), catalog(), &keys).await;
    let token = keys.token("alice");
    let (_, before) = exchange(&router, "GET", &token, Value::Null).await;
    let intent =
        json!({"revision":before["revision"], "changes":[set(app(), "label", json!("broken"))]});
    let (status, error) = exchange(&router, "PATCH", &token, intent).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(error, json!({"error":"unavailable"}));
    assert_eq!(
        exchange(&router, "GET", &token, Value::Null).await.1,
        before
    );
    let intent = json!({"revision":before["revision"],"changes":[
        set(shared(), "label",json!("changed")),
        set(app(), "label",json!(true)),
    ]});
    assert_eq!(
        exchange(&router, "PATCH", &token, intent).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        exchange(&router, "GET", &token, Value::Null).await.1,
        before
    );
}

#[tokio::test]
async fn presence() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), catalog(), &keys).await;
    let token = keys.token("alice");
    let (_, before) = exchange(&router, "GET", &token, Value::Null).await;
    let missing = &before["document"]["app"]["items"]["label"]["app"];
    assert_eq!(missing, &json!({"state":"missing"}));
    let changes = json!({"revision":before["revision"],"changes":[
        set(shared(), "label",json!(null)),
        set(app(), "label",json!("")),
    ]});
    let (status, saved) = exchange(&router, "PATCH", &token, changes).await;
    assert_eq!(status, StatusCode::OK);
    let item = &saved["document"]["app"]["items"]["label"];
    assert_eq!(item["shared"], json!({"state":"present","value":null}));
    assert_eq!(item["app"], json!({"state":"present","value":""}));
    assert_eq!(item["effective"], "");
}
