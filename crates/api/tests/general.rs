mod support;

use axum::http::StatusCode;
use folio::{Catalog, Definition};
use keel_document::Shape;
use serde_json::json;
use support::*;

fn declarative() -> Catalog {
    Catalog::new(
        vec![
            Definition {
                name: "theme.family".into(),
                shape: Shape::Text,
                default: json!("brand"),
                options: vec![json!("brand"), json!("brand-bold")],
            },
            Definition {
                name: "theme.tone".into(),
                shape: Shape::Text,
                default: json!("auto"),
                options: vec![json!("auto"), json!("light")],
            },
        ],
        vec!["ensign".into()],
    )
    .unwrap()
}

#[tokio::test]
async fn unaffected() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), declarative(), &keys).await;
    let token = keys.token("alice");
    let initial = read(&router, &token).await;
    assert!(initial.get("presentation").is_none());
    assert_eq!(item(&initial, "theme.family")["effective"], "brand");
    assert_eq!(item(&initial, "theme.tone")["effective"], "auto");
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("light"))]});
    let (status, changed) = call(
        &router,
        request(
            "PATCH",
            "/v1/me/settings?app=ensign&tone=dark",
            &token,
            body,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert!(changed.get("presentation").is_none());
    assert_eq!(item(&changed, "theme.tone")["effective"], "light");
    assert_eq!(item(&changed, "theme.tone")["source"], "shared");
    let body = json!({"revision":changed["revision"],"changes":[{"op":"reset","scope":shared(),"name":"theme.tone"}]});
    let (status, reset) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reset}");
    assert_eq!(item(&reset, "theme.tone")["effective"], "auto");
}
