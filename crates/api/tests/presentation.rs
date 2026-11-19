mod support;

use axum::http::StatusCode;
use folio::Catalog;
use serde_json::{Value, json};
use support::*;

#[tokio::test]
async fn default() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let body = shown(&router, &keys.token("alice"), "").await;
    assert_eq!(
        body["presentation"],
        json!({"system":"google-accounts","tone":"light","basis":"default"})
    );
    assert_eq!(item(&body, "theme.family")["effective"], "google");
    assert_eq!(item(&body, "theme.tone")["effective"], "system");
    let other = shown(&router, &keys.token("bob"), "").await;
    assert_eq!(other["presentation"], body["presentation"]);
}

#[tokio::test]
async fn device() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let dark = shown(&router, &token, "&tone=dark").await;
    assert_eq!(dark["presentation"]["tone"], "dark");
    assert_eq!(dark["presentation"]["basis"], "device");
    let light = shown(&router, &token, "&tone=light").await;
    assert_eq!(light["presentation"]["tone"], "light");
    assert_eq!(light["presentation"]["basis"], "device");
    let absent = shown(&router, &token, "").await;
    assert_eq!(absent["presentation"]["basis"], "default");
}

#[tokio::test]
async fn rejected() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let initial = shown(&router, &token, "").await;
    for bad in ["auto", "system", "Dark", ""] {
        let path = format!("/v1/me/settings?app=ensign&tone={bad}");
        let (status, body) = call(&router, request("GET", &path, &token, Value::Null)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{bad} {body}");
        assert_eq!(body, json!({"error":"invalid"}));
    }
    assert_eq!(shown(&router, &token, "").await, initial);
}

#[tokio::test]
async fn stable() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let initial = shown(&router, &token, "").await;
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark"))]});
    let (status, changed) = call(
        &router,
        request(
            "PATCH",
            "/v1/me/settings?app=ensign&tone=light",
            &token,
            body,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(
        changed["presentation"],
        json!({"system":"google-accounts","tone":"dark","basis":"setting"})
    );
    let queried = shown(&router, &token, "&tone=light").await;
    assert_eq!(queried["presentation"], changed["presentation"]);
    let other = shown(&router, &token, "&tone=dark").await;
    assert_eq!(other["presentation"]["tone"], "dark");
    assert_eq!(other["presentation"]["basis"], "setting");
}
