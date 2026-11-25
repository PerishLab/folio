use crate::{stored, support::*};
use axum::http::StatusCode;
use folio::Catalog;
use serde_json::{Value, json};

fn note() -> Value {
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

fn strict() -> Value {
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
async fn inspect() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let token = keys.token("alice");
    let (status, _) = call(&router, request("GET", "/v1/templates", "", Value::Null)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, list) = call(
        &router,
        request("GET", "/v1/templates", &token, Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        list["templates"],
        json!([
            {"name":"meta","latest":1,"editions":[1],"builtin":true},
            {"name":"tags","latest":1,"editions":[1],"builtin":true},
            {"name":"theme","latest":1,"editions":[1],"builtin":true}
        ])
    );
    let (status, detail) = call(
        &router,
        request("GET", "/v1/templates/tags", &token, Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["builtin"], true);
    assert_eq!(detail["latest"], 1);
    assert_eq!(detail["editions"][0]["schemaVersion"], 1);
    let (status, edition) = call(
        &router,
        request("GET", "/v1/templates/tags/editions/1", &token, Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(edition["template"], "tags");
    assert_eq!(edition["number"], 1);
    assert_eq!(edition["schemaVersion"], 1);
    assert_eq!(edition["schema"]["required"], json!(["tags"]));
    let (status, _) = call(
        &router,
        request("GET", "/v1/templates/unknown", &token, Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &router,
        request("GET", "/v1/templates/tags/editions/9", &token, Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn manage() {
    let root = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let router = router(root.path(), Catalog::themes(), &keys).await;
    let user = keys.token("alice");
    let admin = keys.token("manager");
    let _ = call(
        &router,
        request("GET", "/v1/templates", &admin, Value::Null),
    )
    .await;
    let create = json!({"name":"notes","schema":note()});
    let (status, body) = call(
        &router,
        request("POST", "/v1/templates", &user, create.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body, json!({"error":"denied"}));
    grant(root.path(), "manager").await;
    let (status, created) = call(
        &router,
        request("POST", "/v1/templates", &admin, create.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["template"], "notes");
    assert_eq!(created["number"], 1);
    let (status, _) = call(&router, request("POST", "/v1/templates", &admin, create)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let bad = json!({"name":"notes-bad","schema":{"type":123}});
    let (status, _) = call(&router, request("POST", "/v1/templates", &admin, bad)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let meta = json!({"base":1,"schema":note()});
    let (status, _) = call(
        &router,
        request("POST", "/v1/templates/meta/editions", &admin, meta),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let stale = json!({"base":9,"schema":strict()});
    let (status, _) = call(
        &router,
        request("POST", "/v1/templates/notes/editions", &admin, stale),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let next = json!({"base":1,"schema":strict()});
    let (status, _) = call(
        &router,
        request("POST", "/v1/templates/notes/editions", &user, next.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, second) = call(
        &router,
        request("POST", "/v1/templates/notes/editions", &admin, next),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{second}");
    assert_eq!(second["number"], 2);
}

#[tokio::test]
async fn status() {
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
    let create = json!({"name":"notes","schema":note()});
    assert_eq!(
        call(&router, request("POST", "/v1/templates", &admin, create))
            .await
            .0,
        StatusCode::CREATED
    );
    let first = json!({"id":"good","template":"notes","document":{"title":"A","summary":"B"}});
    let second = json!({"id":"draft","template":"notes","document":{"title":"Only"}});
    assert_eq!(
        stored(&router, request("POST", "/v1/me/documents", &alice, first))
            .await
            .0,
        StatusCode::CREATED
    );
    assert_eq!(
        stored(&router, request("POST", "/v1/me/documents", &alice, second))
            .await
            .0,
        StatusCode::CREATED
    );
    let upgrade = json!({"base":1,"schema":strict()});
    assert_eq!(
        call(
            &router,
            request("POST", "/v1/templates/notes/editions", &admin, upgrade),
        )
        .await
        .0,
        StatusCode::CREATED
    );
    let (code, report) = stored(
        &router,
        request("GET", "/v1/templates/notes/status", &alice, Value::Null),
    )
    .await;
    assert_eq!(code, StatusCode::OK, "{report}");
    assert_eq!(report["template"], "notes");
    assert_eq!(report["latest"], 2);
    assert_eq!(report["format"], "folio:2");
    assert_eq!(report["ready"], false);
    assert_eq!(report["checked"], 2);
    assert_eq!(report["compatible"], 1);
    assert_eq!(report["incompatible"].as_array().unwrap().len(), 1);
    assert_eq!(report["incompatible"][0]["id"], "draft");
    assert_eq!(report["incompatible"][0]["edition"], 1);
    let (code, isolated) = stored(
        &router,
        request("GET", "/v1/templates/notes/status", &bob, Value::Null),
    )
    .await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(isolated["checked"], 0);
    assert_eq!(isolated["ready"], true);
}
