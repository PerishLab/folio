mod support;

use axum::http::StatusCode;
use folio::{Catalog, Definition};
use serde_json::json;
use support::*;

#[tokio::test]
async fn patch() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let catalog = Catalog::new(
        vec![Definition {
            name: "data.sample".into(),
            shape: keel_document::Shape::Any,
            default: json!({}),
            options: vec![],
        }],
        vec!["ensign".into()],
    )
    .unwrap();
    let router = router(root.path(), catalog, &keys).await;
    let token = keys.token("alice");
    let initial = read(&router, &token).await;
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"data.sample",json!({"a.b":[{"a/b":null,"~":false}]}))]});
    let (status, first) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let body = json!({"revision":first["revision"],"changes":[{"op":"patch","name":"data.sample","scope":shared(),"operations":[{"op":"remove","path":"/a.b/0/a~1b"},{"op":"set","path":"/a.b/0/~0","value":0}]}]});
    let (status, second) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        item(&second, "data.sample")["effective"],
        json!({"a.b":[{"~":0}]})
    );
    let body = json!({"revision":second["revision"],"changes":[{"op":"patch","name":"data.sample","scope":shared(),"operations":[{"op":"set","path":"/a.b/0/~0","value":true},{"op":"remove","path":"/missing"}]}]});
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(read(&router, &token).await, second);
}

#[tokio::test]
async fn writers() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let initial = read(&router, &token).await;
    let left = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("light"))]});
    let right = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark"))]});
    let left = request("PATCH", "/v1/me/settings?app=ensign", &token, left);
    let right = request("PATCH", "/v1/me/settings?app=ensign", &token, right);
    let (left, right) = tokio::join!(call(&router, left), call(&router, right));
    let (winner, loser) = if left.0 == StatusCode::OK {
        (left, right)
    } else {
        (right, left)
    };
    assert_eq!(winner.0, StatusCode::OK);
    assert_eq!(loser.0, StatusCode::CONFLICT);
    assert_eq!(read(&router, &token).await, winner.1);
}

#[tokio::test]
async fn revoke() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let initial = read(&router, &token).await;
    let mut graph = keel::Graph::new();
    folio::stock(&mut graph);
    let core = keel::bind(
        graph,
        keel::adapt::db::Sqlite::file(&root.path().join("estate.sqlite3"))
            .await
            .unwrap(),
    )
    .await
    .unwrap()
    .identify("Actor")
    .unwrap();
    let rows = core.live("@grant").await.unwrap();
    let grant = rows
        .iter()
        .find(|row| {
            row.text("unit") == Some("documentstore:document") && row.text("verb") == Some("*")
        })
        .unwrap();
    core.end("@grant", grant.key()).await.unwrap();
    let body = json!({"revision":initial["revision"],"changes":[set(shared(),"theme.tone",json!("dark"))]});
    let (status, _) = call(
        &router,
        request("PATCH", "/v1/me/settings?app=ensign", &token, body),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
