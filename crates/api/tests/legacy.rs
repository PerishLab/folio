mod support;

use axum::http::StatusCode;
use folio::{Catalog, Runtime};
use serde_json::json;
use support::*;

#[tokio::test]
async fn stored() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let runtime = Runtime::create(root.path(), Catalog::themes(), ISSUER.into())
        .await
        .unwrap();
    let router = api::router(runtime, keys.auth());
    let token = keys.token("legacy");
    let initial = shown(&router, &token, "").await;
    let body = json!({"revision":initial["revision"],"changes":[
        set(shared(),"theme.family",json!("google")),
        set(shared(),"theme.tone",json!("light"))
    ]});
    let (status, written) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{written}");
    let state = read(&router, &token).await;
    assert_eq!(item(&state, "theme.family")["effective"], "google");
    assert_eq!(item(&state, "theme.family")["source"], "shared");
    assert_eq!(
        state["presentation"],
        json!({"system":"google-accounts","tone":"light","basis":"setting"})
    );
    let body = json!({"revision":state["revision"],"changes":[set(shared(),"theme.family",json!("google-workspace"))]});
    let (status, moved) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["presentation"]["system"], "google-workspace");
}
