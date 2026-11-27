use crate::{stored, support::*};
use axum::http::StatusCode;
use folio::Catalog;
use serde_json::{Value, json};

const BASE: &str = "/v1/me/documents";

fn first() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "title": {"type": "string", "minLength": 1},
            "summary": {"type": "string"}
        },
        "required": ["title"],
        "additionalProperties": false
    })
}

fn second() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": {
            "title": {"type": "string", "minLength": 1},
            "summary": {"type": "string", "minLength": 1}
        },
        "required": ["title", "summary"],
        "additionalProperties": false
    })
}

#[tokio::test]
async fn upgrade() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let alice = keys.token("alice");
    let admin = keys.token("manager");
    let _ = call(
        &router,
        request("GET", "/v1/templates", &admin, Value::Null),
    )
    .await;
    grant(root.path(), "manager").await;
    let schema = json!({"name":"notes","schema":first()});
    assert_eq!(
        call(&router, request("POST", "/v1/templates", &admin, schema))
            .await
            .0,
        StatusCode::CREATED
    );
    let draft = json!({"id":"plan","template":"notes","document":{"title":"V1"}});
    let (_, created) = stored(&router, request("POST", BASE, &alice, draft)).await;
    let next = json!({"base":1,"schema":second()});
    assert_eq!(
        call(
            &router,
            request("POST", "/v1/templates/notes/editions", &admin, next),
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let ask = json!({"revision": created["revision"]});
    let (status, _) = stored(
        &router,
        request("POST", "/v1/me/documents/plan/migrate", &alice, ask),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, still) = stored(
        &router,
        request("GET", "/v1/me/documents/plan", &alice, Value::Null),
    )
    .await;
    assert_eq!(still["edition"], 1);
    let patch = json!({
        "revision": still["revision"],
        "operations": [{"op":"set","path":"/summary","value":"Ready"}]
    });
    let (status, filled) = stored(
        &router,
        request("PATCH", "/v1/me/documents/plan", &alice, patch),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let stale = json!({"revision": created["revision"]});
    assert_eq!(
        stored(
            &router,
            request("POST", "/v1/me/documents/plan/migrate", &alice, stale),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let ok = json!({"revision": filled["revision"]});
    let (status, moved) = stored(
        &router,
        request("POST", "/v1/me/documents/plan/migrate", &alice, ok),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{moved}");
    assert_eq!(moved["edition"], 2);
    assert_ne!(moved["revision"], filled["revision"]);
    let stale = json!({
        "revision": filled["revision"],
        "operations": [{"op":"set","path":"/title","value":"Stale"}]
    });
    assert_eq!(
        stored(
            &router,
            request("PATCH", "/v1/me/documents/plan", &alice, stale),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let drop = json!({
        "revision": moved["revision"],
        "operations": [{"op":"remove","path":"/summary"}]
    });
    assert_eq!(
        stored(
            &router,
            request("PATCH", "/v1/me/documents/plan", &alice, drop),
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        call(
            &router,
            request("GET", "/v1/me/documents?subject=alice", &alice, Value::Null),
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn revoke() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let alice = keys.token("alice");
    let tags = json!({"a":{"label":"A"}});
    let draft = json!({"id":"mine","template":"tags","document":{"tags":tags}});
    let (status, created) = stored(&router, request("POST", BASE, &alice, draft)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let mut graph = keel::Graph::new();
    folio::stock(&mut graph);
    let wire = keel::adapt::db::Sqlite::file(&root.path().join("estate.sqlite3"))
        .await
        .unwrap();
    let core = keel::bind(graph, wire)
        .await
        .unwrap()
        .identify("Actor")
        .unwrap();
    for row in core.live("@grant").await.unwrap() {
        let doc =
            row.text("unit") == Some("documentstore:document") && row.text("verb") == Some("*");
        if !doc {
            continue;
        }
        let who = row.text("who").unwrap().to_string();
        let scope = row.text("scope").unwrap().to_string();
        core.revoke(row.key()).await.unwrap();
        core.grant(keel::Grant::new(
            who.as_str(),
            "see",
            "documentstore:document",
            scope.as_str(),
        ))
        .await
        .unwrap();
    }
    let patch = json!({
        "revision": created["revision"],
        "operations": [{"op":"set","path":"/tags/b","value":{"label":"B"}}]
    });
    assert_eq!(
        stored(
            &router,
            request("PATCH", "/v1/me/documents/mine", &alice, patch)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let rev = created["revision"].as_str().unwrap();
    let path = format!("/v1/me/documents/mine?revision={rev}");
    assert_eq!(
        stored(&router, request("DELETE", &path, &alice, Value::Null))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn race() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let alice = keys.token("alice");
    let admin = keys.token("manager");
    let _ = call(
        &router,
        request("GET", "/v1/templates", &admin, Value::Null),
    )
    .await;
    grant(root.path(), "manager").await;
    let schema = json!({"name":"notes","schema":first()});
    assert_eq!(
        call(&router, request("POST", "/v1/templates", &admin, schema))
            .await
            .0,
        StatusCode::CREATED
    );
    let draft = json!({"id":"plan","template":"notes","document":{"title":"V1","summary":"Ok"}});
    let (_, created) = stored(&router, request("POST", BASE, &alice, draft)).await;
    let next = json!({"base":1,"schema":second()});
    assert_eq!(
        call(
            &router,
            request("POST", "/v1/templates/notes/editions", &admin, next),
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let patch = json!({
        "revision": created["revision"],
        "operations": [{"op":"remove","path":"/summary"}]
    });
    let ask = json!({"revision": created["revision"]});
    let left = stored(
        &router,
        request("PATCH", "/v1/me/documents/plan", &alice, patch),
    );
    let right = stored(
        &router,
        request("POST", "/v1/me/documents/plan/migrate", &alice, ask),
    );
    let (patched, migrated) = tokio::join!(left, right);
    assert!(
        matches!(
            (patched.0, migrated.0),
            (StatusCode::OK, StatusCode::CONFLICT) | (StatusCode::CONFLICT, StatusCode::OK)
        ),
        "patched={patched:?} migrated={migrated:?}"
    );
    let (_, current) = stored(
        &router,
        request("GET", "/v1/me/documents/plan", &alice, Value::Null),
    )
    .await;
    if current["edition"] == 2 {
        assert_eq!(current["document"]["summary"], "Ok");
    } else {
        assert_eq!(current["edition"], 1);
        assert!(current["document"].get("summary").is_none());
    }
}
