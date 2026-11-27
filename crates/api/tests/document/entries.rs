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

#[tokio::test]
async fn crud() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let alice = keys.token("alice");
    let bob = keys.token("bob");
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
    for blocked in ["preferences", "workbench", "tags"] {
        let body = json!({"id":blocked,"template":"notes","document":{"title":"A"}});
        assert_eq!(
            stored(&router, request("POST", BASE, &alice, body)).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let meta = json!({"id":"m","template":"meta","document":{"schemaVersion":1,"schema":{}}});
    assert_eq!(
        stored(&router, request("POST", BASE, &alice, meta)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let bad = json!({"id":"n1","template":"notes","document":{"title":""}});
    assert_eq!(
        stored(&router, request("POST", BASE, &alice, bad)).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let _ = stored(&router, request("GET", BASE, &alice, Value::Null)).await;
    let _ = stored(&router, request("GET", BASE, &bob, Value::Null)).await;
    let before = estate(root.path())
        .await
        .live("@grant")
        .await
        .unwrap()
        .len();
    let draft = json!({"id":"roadmap","template":"notes","document":{"title":"Q1"}});
    let (status, created) = stored(&router, request("POST", BASE, &alice, draft.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["id"], "roadmap");
    assert_eq!(created["template"], "notes");
    assert_eq!(created["edition"], 1);
    assert_eq!(
        stored(&router, request("POST", BASE, &alice, draft.clone()))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (status, listed) = stored(&router, request("GET", BASE, &alice, Value::Null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["documents"].as_array().unwrap().len(), 1);
    assert_eq!(listed["documents"][0]["id"], "roadmap");
    let (status, empty) = stored(&router, request("GET", BASE, &bob, Value::Null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["documents"], json!([]));
    assert_eq!(
        stored(
            &router,
            request("GET", "/v1/me/documents/roadmap", &bob, Value::Null),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let rev = created["revision"].as_str().unwrap();
    let steal = format!("/v1/me/documents/roadmap?revision={rev}");
    assert_eq!(
        stored(&router, request("DELETE", &steal, &bob, Value::Null))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        stored(
            &router,
            request("DELETE", "/v1/me/documents/roadmap", &alice, Value::Null),
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        stored(
            &router,
            request(
                "DELETE",
                "/v1/me/documents/roadmap?revision=",
                &alice,
                Value::Null,
            ),
        )
        .await,
        (StatusCode::UNPROCESSABLE_ENTITY, json!({"error":"invalid"}))
    );
    assert_eq!(
        stored(
            &router,
            request(
                "DELETE",
                "/v1/me/documents/roadmap",
                &alice,
                json!({"revision":""}),
            ),
        )
        .await,
        (StatusCode::UNPROCESSABLE_ENTITY, json!({"error":"invalid"}))
    );
    let missing = json!({"id":"x","template":"missing","document":{}});
    assert_eq!(
        stored(&router, request("POST", BASE, &alice, missing))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, bobs) = stored(&router, request("POST", BASE, &bob, draft)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_ne!(bobs["revision"], created["revision"]);
    let patch = json!({
        "revision": created["revision"],
        "operations": [{"op":"set","path":"/title","value":"Q2"}]
    });
    let (status, edited) = stored(
        &router,
        request("PATCH", "/v1/me/documents/roadmap", &alice, patch.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{edited}");
    assert_eq!(edited["document"]["title"], "Q2");
    assert_eq!(
        stored(
            &router,
            request("PATCH", "/v1/me/documents/roadmap", &alice, patch),
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let stale = format!("/v1/me/documents/roadmap?revision={rev}");
    assert_eq!(
        stored(&router, request("DELETE", &stale, &alice, Value::Null))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let next = edited["revision"].as_str().unwrap();
    let current = format!("/v1/me/documents/roadmap?revision={next}");
    assert_eq!(
        stored(&router, request("DELETE", &current, &bob, Value::Null))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        stored(&router, request("DELETE", &current, &alice, Value::Null))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        stored(
            &router,
            request("GET", "/v1/me/documents/roadmap", &alice, Value::Null),
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let body = json!({"revision": bobs["revision"]});
    assert_eq!(
        stored(
            &router,
            request("DELETE", "/v1/me/documents/roadmap", &bob, body),
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    let after = estate(root.path())
        .await
        .live("@grant")
        .await
        .unwrap()
        .len();
    assert_eq!(after, before);
}

#[tokio::test]
async fn denied() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let alice = keys.token("alice");
    let _ = stored(&router, request("GET", BASE, &alice, Value::Null)).await;
    let core = estate(root.path()).await;
    for row in core.live("@grant").await.unwrap() {
        if row.text("unit") == Some("Entry") && row.text("verb") == Some("put") {
            core.revoke(row.key()).await.unwrap();
        }
    }
    let docs = core.live("documentstore:document").await.unwrap().len();
    let grants = core.live("@grant").await.unwrap().len();
    let draft = json!({"id":"blocked","template":"tags","document":{"tags":{}}});
    let (status, body) = stored(&router, request("POST", BASE, &alice, draft)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body, json!({"error":"denied"}));
    assert_eq!(
        core.live("documentstore:document").await.unwrap().len(),
        docs
    );
    assert_eq!(core.live("@grant").await.unwrap().len(), grants);
}

async fn estate(root: &std::path::Path) -> keel::Core<keel::adapt::db::Sqlite> {
    let mut graph = keel::Graph::new();
    folio::stock(&mut graph);
    let wire = keel::adapt::db::Sqlite::file(&root.join("estate.sqlite3"))
        .await
        .unwrap();
    keel::bind(graph, wire)
        .await
        .unwrap()
        .identify("Actor")
        .unwrap()
}
