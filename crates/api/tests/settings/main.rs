#[path = "../support/mod.rs"]
mod support;

mod legacy;

use axum::http::StatusCode;
use folio::{Catalog, Decision, Runtime};
use serde_json::json;
use support::*;

#[tokio::test]
async fn layers() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("external-9000");
    let initial = read(&router, &token).await;
    assert_eq!(item(&initial, "theme.tone")["effective"], "system");
    assert_eq!(item(&initial, "theme.tone")["shared"]["state"], "missing");
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark")),set(app(),"theme.tone",json!("light"))]});
    let (status, changed) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{changed}");
    assert_eq!(item(&changed, "theme.tone")["effective"], "light");
    assert_eq!(item(&changed, "theme.tone")["source"], "app");
    assert_eq!(item(&changed, "theme.tone")["shared"]["value"], "dark");
    let body = json!({"revision":changed["revision"],"changes":[{"op":"reset","scope":app(),"name":"theme.tone"}]});
    let (status, reset) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item(&reset, "theme.tone")["effective"], "dark");
    assert_eq!(item(&reset, "theme.tone")["source"], "shared");
    let (status, one) = call(
        &router,
        request(
            "GET",
            "/v1/me/settings/theme.tone?app=ensign",
            &token,
            json!(null),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(one["item"], *item(&reset, "theme.tone"));
    let other = read(&router, &keys.token("another-subject")).await;
    assert_eq!(item(&other, "theme.tone")["effective"], "system");
}

#[tokio::test]
async fn atomic() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let initial = read(&router, &token).await;
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark")),set(app(),"theme.family",json!("unknown"))]});
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(read(&router, &token).await, initial);
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark"))]});
    let (status, changed) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(read(&router, &token).await, changed);
}

#[tokio::test]
async fn policy() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let catalog = Catalog::themes().policy(|context, definition| {
        if context.identity.subject() == "managed" && definition.name == "theme.tone" {
            Decision {
                force: Some(json!("light")),
                options: Some(vec![json!("light")]),
                readonly: true,
            }
        } else {
            Decision::default()
        }
    });
    let router = router(root.path(), catalog, &keys).await;
    let token = keys.token("managed");
    let initial = read(&router, &token).await;
    let value = item(&initial, "theme.tone");
    assert_eq!(value["source"], "policy");
    assert_eq!(value["editable"], false);
    assert_eq!(value["effective"], "light");
    assert_eq!(value["options"], json!(["light"]));
    let body =
        json!({"revision":initial["revision"],"changes":[set(app(),"theme.tone",json!("dark"))]});
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(read(&router, &token).await, initial);
}

#[tokio::test]
async fn durable() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let token = keys.token("90000");
    let runtime = Runtime::create(root.path(), Catalog::themes(), ISSUER.into())
        .await
        .unwrap();
    let identity = keys.auth().verify(&token).unwrap();
    let actor = runtime.operator(&identity).await.unwrap();
    assert_ne!(actor, 90000);
    let router = api::router(runtime, keys.auth());
    let initial = read(&router, &token).await;
    let body =
        json!({"revision":initial["revision"],"changes":[set(app(),"theme.tone",json!("dark"))]});
    let (status, changed) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    drop(router);
    let runtime = Runtime::open(root.path(), Catalog::themes(), ISSUER.into())
        .await
        .unwrap();
    assert_eq!(runtime.operator(&identity).await.unwrap(), actor);
    assert_eq!(
        read(&api::router(runtime, keys.auth()), &token).await,
        changed
    );
}

#[tokio::test]
async fn derived() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let catalog = Catalog::themes().policy(|context, definition| {
        if definition.name == "theme.family"
            && context.values.pointer("/shared/theme.tone") == Some(&json!("dark"))
        {
            Decision {
                force: Some(json!("material")),
                ..Decision::default()
            }
        } else {
            Decision::default()
        }
    });
    let router = router(root.path(), catalog, &keys).await;
    let token = keys.token("alice");
    let initial = read(&router, &token).await;
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark"))]});
    let (status, changed) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(item(&changed, "theme.family")["effective"], "material");
    assert_eq!(item(&changed, "theme.family")["source"], "policy");
    assert_eq!(item(&changed, "theme.family")["shared"]["state"], "missing");
}

#[tokio::test]
async fn choices() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let catalog = Catalog::themes().policy(|_, definition| {
        if definition.name == "theme.tone" {
            Decision {
                options: Some(vec![json!("dark")]),
                ..Decision::default()
            }
        } else {
            Decision::default()
        }
    });
    let router = router(root.path(), catalog, &keys).await;
    let initial = read(&router, &keys.token("alice")).await;
    assert_eq!(item(&initial, "theme.tone")["effective"], "dark");
    assert_eq!(item(&initial, "theme.tone")["source"], "policy");
    assert_eq!(item(&initial, "theme.tone")["options"], json!(["dark"]));
}

#[tokio::test]
async fn candidate() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let catalog = Catalog::themes().policy(|context, definition| {
        if definition.name == "theme.family"
            && context.values.pointer("/shared/theme.tone") == Some(&json!("dark"))
        {
            Decision {
                options: Some(vec![]),
                ..Decision::default()
            }
        } else {
            Decision::default()
        }
    });
    let router = router(root.path(), catalog, &keys).await;
    let token = keys.token("alice");
    let initial = read(&router, &token).await;
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark"))]});
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(read(&router, &token).await, initial);
}
