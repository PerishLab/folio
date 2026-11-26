use crate::{stored, support::*};
use axum::http::StatusCode;
use folio::{Catalog, Decision};
use serde_json::{Value, json};

const BOARD: &str = "/v1/me/documents/workbench?tone=dark";

#[tokio::test]
async fn scopes() {
    assert!(Catalog::themes().extend(vec!["Bad_App".into()]).is_err());
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let catalog = Catalog::themes().extend(vec!["santi".into()]).unwrap();
    let router = router(root.path(), catalog, &keys).await;
    let token = keys.token("alice");
    let (status, _) = call(&router, request("GET", "/v1/scopes", "", Value::Null)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = call(
        &router,
        request("GET", "/v1/scopes?subject=alice", &token, Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = call(&router, request("GET", "/v1/scopes", &token, Value::Null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["scopes"],
        json!([
            {"kind":"shared","name":null,"builtin":true},
            {"kind":"app","name":"ensign","builtin":true},
            {"kind":"app","name":"folio","builtin":true},
            {"kind":"app","name":"santi","builtin":false}
        ])
    );
    assert_eq!(body["definitions"].as_array().unwrap().len(), 2);
    let (health, _) = call(&router, request("GET", "/api/health", "", Value::Null)).await;
    assert_eq!(health, StatusCode::OK);
    let (nested, prefixed) = call(
        &router,
        request("GET", "/api/v1/scopes", &token, Value::Null),
    )
    .await;
    assert_eq!(nested, StatusCode::OK);
    assert_eq!(prefixed, body);
}

#[tokio::test]
async fn crossapp() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let catalog = Catalog::themes().extend(vec!["santi".into()]).unwrap();
    let router = router(root.path(), catalog, &keys).await;
    let token = keys.token("alice");
    let (status, initial) = stored(&router, request("GET", BOARD, &token, Value::Null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        initial["document"]["scopes"],
        json!(["ensign", "folio", "santi"])
    );
    assert_eq!(
        initial["document"].pointer("/apps/folio/items/theme.tone/effective"),
        Some(&json!("system"))
    );
    assert_eq!(
        initial["document"].pointer("/apps/folio/presentation/tone"),
        Some(&json!("dark"))
    );
    let folio = json!({"kind":"app","name":"folio"});
    let santi = json!({"kind":"app","name":"santi"});
    let intent = json!({
        "revision": initial["revision"],
        "changes": [
            set(shared(), "theme.tone", json!("light")),
            set(folio.clone(), "theme.tone", json!("dark")),
            set(santi.clone(), "theme.family", json!("material"))
        ]
    });
    let (status, saved) = stored(&router, request("PATCH", BOARD, &token, intent.clone())).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_ne!(saved["revision"], initial["revision"]);
    assert_eq!(
        saved["document"].pointer("/shared/items/theme.tone/source"),
        Some(&json!("shared"))
    );
    assert_eq!(
        saved["document"].pointer("/apps/ensign/items/theme.tone/effective"),
        Some(&json!("light"))
    );
    assert_eq!(
        saved["document"].pointer("/apps/ensign/items/theme.tone/source"),
        Some(&json!("shared"))
    );
    assert_eq!(
        saved["document"].pointer("/apps/folio/items/theme.tone/effective"),
        Some(&json!("dark"))
    );
    assert_eq!(
        saved["document"].pointer("/apps/folio/items/theme.tone/source"),
        Some(&json!("app"))
    );
    assert_eq!(
        saved["document"].pointer("/apps/santi/items/theme.family/effective"),
        Some(&json!("material"))
    );
    let (status, _) = stored(&router, request("PATCH", BOARD, &token, intent)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let reset = json!({
        "revision": saved["revision"],
        "changes": [{"op":"reset","scope":folio,"name":"theme.tone"}]
    });
    let (status, back) = stored(&router, request("PATCH", BOARD, &token, reset)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        back["document"].pointer("/apps/folio/items/theme.tone/effective"),
        Some(&json!("light"))
    );
    assert_eq!(
        back["document"].pointer("/apps/folio/items/theme.tone/source"),
        Some(&json!("shared"))
    );
    let (status, _) = call(
        &router,
        request(
            "GET",
            "/v1/me/documents/workbench?subject=alice",
            &token,
            Value::Null,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = stored(
        &router,
        request(
            "GET",
            "/v1/me/documents/workbench?tone=bad",
            &token,
            Value::Null,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn policy() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let catalog = Catalog::themes().policy(|context, definition| {
        if context.app == Some("folio") && definition.name == "theme.tone" {
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
    let token = keys.token("alice");
    let (_, initial) = stored(&router, request("GET", BOARD, &token, Value::Null)).await;
    assert_eq!(
        initial["document"].pointer("/apps/folio/items/theme.tone/source"),
        Some(&json!("policy"))
    );
    assert_eq!(
        initial["document"].pointer("/apps/folio/items/theme.tone/editable"),
        Some(&json!(false))
    );
    let folio = json!({"kind":"app","name":"folio"});
    let intent = json!({
        "revision": initial["revision"],
        "changes": [set(folio, "theme.tone", json!("dark"))]
    });
    let (status, body) = stored(&router, request("PATCH", BOARD, &token, intent)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body, json!({"error":"denied"}));
    assert_eq!(
        stored(&router, request("GET", BOARD, &token, Value::Null))
            .await
            .1,
        initial
    );
}

#[test]
fn config() {
    let dir = tempfile::tempdir().unwrap();
    let toml = concat!(
        "issuer = \"https://identity.test/api\"\n",
        "audience = \"folio\"\n",
        "apps = [\"santi\", \"concord\"]\n",
    );
    std::fs::write(dir.path().join("folio.toml"), toml).unwrap();
    let loaded = api::config::Settings::load(dir.path()).unwrap();
    assert_eq!(loaded.apps, vec!["santi", "concord"]);
    assert_eq!(
        loaded.catalog().unwrap().apps(),
        &["ensign", "folio", "santi", "concord"]
    );
    let primary = api::config::Settings::from(dir.path(), &|key| match key {
        "FOLIO_APPS" => Some("gamma, delta".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(primary.apps, vec!["gamma", "delta"]);
    assert_eq!(
        primary.catalog().unwrap().apps(),
        &["ensign", "folio", "gamma", "delta"]
    );
    let legacy = api::config::Settings::from(dir.path(), &|key| match key {
        "API_APPS" => Some("alpha, beta".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(legacy.apps, vec!["alpha", "beta"]);
    assert_eq!(
        legacy.catalog().unwrap().apps(),
        &["ensign", "folio", "alpha", "beta"]
    );
    let both = api::config::Settings::from(dir.path(), &|key| match key {
        "FOLIO_APPS" => Some("gamma".into()),
        "API_APPS" => Some("alpha".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(both.apps, vec!["gamma"]);
    let blank = api::config::Settings::from(dir.path(), &|key| match key {
        "FOLIO_APPS" => Some("   ".into()),
        "API_APPS" => Some("alpha".into()),
        _ => None,
    })
    .unwrap();
    assert_eq!(blank.apps, vec!["alpha"]);
    let sidecar = api::config::Settings::from(dir.path(), &|key| {
        if key == "SIDECAR_PORT" {
            Some("4312".into())
        } else {
            None
        }
    })
    .unwrap();
    assert_eq!(sidecar.listen, "127.0.0.1:4312");
}
