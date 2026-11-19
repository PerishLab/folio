mod support;

use axum::http::StatusCode;
use folio::Catalog;
use serde_json::{Value, json};
use support::*;

const SHELL: &str = r#"{"props":{"tone":["light","dark"],"system":"string"},
    "systems":["google-accounts","google-workspace","material"]}"#;

#[tokio::test]
async fn render() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let contract: Value = serde_json::from_str(SHELL).unwrap();
    let systems = contract["systems"].as_array().unwrap();
    let tones = contract["props"]["tone"].as_array().unwrap();
    let expected = [
        ("google", "google-accounts"),
        ("google-accounts", "google-accounts"),
        ("google-workspace", "google-workspace"),
        ("material", "material"),
    ];
    let mut revision = shown(&router, &token, "").await["revision"].clone();
    for (family, system) in expected {
        let body = json!({"revision":revision,"changes":[set(app(),"theme.family",json!(family))]});
        let (status, changed) = call(
            &router,
            request("PATCH", "/v1/me/settings?app=ensign", &token, body),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{family} {changed}");
        assert_eq!(item(&changed, "theme.family")["effective"], family);
        let rendered = &changed["presentation"]["system"];
        assert_eq!(rendered, system);
        assert!(systems.iter().any(|key| key == rendered), "{family}");
        assert!(
            tones
                .iter()
                .any(|key| key == &changed["presentation"]["tone"])
        );
        revision = changed["revision"].clone();
    }
}
