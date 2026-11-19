mod support;

use folio::{Catalog, Decision};
use serde_json::json;
use support::*;

fn managed(catalog: Catalog) -> Catalog {
    catalog.policy(|context, definition| {
        if definition.name == "theme.tone" && context.identity.subject() == "managed" {
            Decision {
                force: Some(json!("dark")),
                options: Some(vec![json!("dark")]),
                readonly: true,
            }
        } else {
            Decision::default()
        }
    })
}

#[tokio::test]
async fn forced() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), managed(Catalog::themes()), &keys).await;
    let body = shown(&router, &keys.token("managed"), "&tone=light").await;
    assert_eq!(
        body["presentation"],
        json!({"system":"google-accounts","tone":"dark","basis":"setting"})
    );
    assert_eq!(item(&body, "theme.tone")["source"], "policy");
    assert_eq!(item(&body, "theme.tone")["editable"], false);
}

#[tokio::test]
async fn layered() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let initial = shown(&router, &token, "").await;
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("system"))]});
    let (status, changed) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, 200, "{changed}");
    assert_eq!(changed["presentation"]["basis"], "default");
    let dark = shown(&router, &token, "&tone=dark").await;
    assert_eq!(dark["presentation"]["tone"], "dark");
    let body =
        json!({"revision":dark["revision"],"changes":[set(app(),"theme.tone",json!("light"))]});
    let (status, overlaid) = call(
        &router,
        request(
            "PATCH",
            "/v1/me/settings?app=ensign&tone=dark",
            &token,
            body,
        ),
    )
    .await;
    assert_eq!(status, 200, "{overlaid}");
    assert_eq!(overlaid["presentation"]["tone"], "light");
    assert_eq!(overlaid["presentation"]["basis"], "setting");
    let body = json!({"revision":overlaid["revision"],"changes":[{"op":"reset","scope":app(),"name":"theme.tone"}]});
    let (status, reset) = call(
        &router,
        request(
            "PATCH",
            "/v1/me/settings?app=ensign&tone=light",
            &token,
            body,
        ),
    )
    .await;
    assert_eq!(status, 200, "{reset}");
    assert_eq!(item(&reset, "theme.tone")["source"], "shared");
    assert_eq!(
        reset["presentation"],
        json!({"system":"google-accounts","tone":"light","basis":"device"})
    );
    let other = shown(&router, &keys.token("carol"), "&tone=dark").await;
    assert_eq!(other["presentation"]["tone"], "dark");
    assert_eq!(other["presentation"]["basis"], "device");
}

#[tokio::test]
async fn refusal() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let initial = shown(&router, &token, "").await;
    let body = json!({"revision":initial["revision"],"changes":[set(app(),"theme.family",json!("unknown"))]});
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, 422);
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark"))]});
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body.clone()),
    )
    .await;
    assert_eq!(status, 200);
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, 409);
    let now = shown(&router, &token, "&tone=dark").await;
    assert_eq!(
        now["presentation"],
        json!({"system":"google-accounts","tone":"dark","basis":"setting"})
    );
}
