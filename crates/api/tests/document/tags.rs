use crate::{stored, support::*};
use axum::http::StatusCode;
use folio::Catalog;
use serde_json::{Value, json};

const ROUTE: &str = "/v1/me/documents/tags";

#[tokio::test]
async fn lifecycle() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let alice = keys.token("alice");
    let bob = keys.token("bob");
    let (status, _) = stored(&router, request("GET", ROUTE, "", Value::Null)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, initial) = stored(&router, request("GET", ROUTE, &alice, Value::Null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["template"], "tags");
    assert_eq!(initial["edition"], 1);
    assert_eq!(initial["document"], json!({"tags":{}}));
    let patch = json!({
        "revision": initial["revision"],
        "operations": [
            {"op":"set","path":"/tags/work","value":{"label":"Work","color":"#2563eb"}},
            {"op":"set","path":"/tags/home","value":{"label":"Home"}}
        ]
    });
    let (status, saved) = stored(&router, request("PATCH", ROUTE, &alice, patch.clone())).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_ne!(saved["revision"], initial["revision"]);
    assert_eq!(saved["document"]["tags"]["work"]["label"], "Work");
    let (status, _) = stored(&router, request("PATCH", ROUTE, &alice, patch)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let remove = json!({
        "revision": saved["revision"],
        "operations": [{"op":"remove","path":"/tags/home"}]
    });
    let (status, trimmed) = stored(&router, request("PATCH", ROUTE, &alice, remove)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        trimmed["document"],
        json!({"tags":{"work":{"label":"Work","color":"#2563eb"}}})
    );
    let invalid = json!({
        "revision": trimmed["revision"],
        "operations": [{"op":"set","path":"/tags/bad","value":{"label":""}}]
    });
    let (status, _) = stored(&router, request("PATCH", ROUTE, &alice, invalid)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, other) = stored(&router, request("GET", ROUTE, &bob, Value::Null)).await;
    assert_eq!(other["document"], json!({"tags":{}}));
    let hijack = json!({
        "revision": trimmed["revision"],
        "operations": [{"op":"set","path":"/tags/x","value":{"label":"X"}}]
    });
    let (status, _) = stored(&router, request("PATCH", ROUTE, &bob, hijack)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        stored(&router, request("GET", ROUTE, &alice, Value::Null))
            .await
            .1,
        trimmed
    );
}
