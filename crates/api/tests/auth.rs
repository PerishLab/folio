mod support;

use axum::http::StatusCode;
use folio::Catalog;
use radicula_oidc::jwt::Audience;
use serde_json::json;
use support::*;

#[tokio::test]
async fn identity() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let mut variants = Vec::new();
    let mut claims = keys.claims("alice");
    claims.iss = "https://foreign.test".into();
    variants.push(claims);
    let mut claims = keys.claims("alice");
    claims.aud = Audience::One("https://wrong.test".into());
    variants.push(claims);
    let mut claims = keys.claims("alice");
    claims.iat -= 1000;
    claims.exp = claims.iat + 1;
    variants.push(claims);
    let mut claims = keys.claims("alice");
    claims.extra.insert("kind".into(), json!("id"));
    variants.push(claims);
    for claims in variants {
        let token = keys.signer.sign(&claims).unwrap();
        let (status, _) = call(
            &router,
            request("GET", "/v1/me/settings", token.expose(), json!(null)),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = call(
        &router,
        request("GET", "/v1/me/settings", "invalid", json!(null)),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn scope() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let initial = read(&router, &token).await;
    let body = json!({"revision":initial["revision"],"changes":[set(json!({"kind":"app","name":"foreign"}),"theme.tone",json!("dark"))]});
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(read(&router, &token).await, initial);
    let (status, _) = call(
        &router,
        request("GET", "/v1/me/settings?subject=other", &token, json!(null)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
