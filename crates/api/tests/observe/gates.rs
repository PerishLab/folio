use crate::{PRODUCER, atom, gzip, postreq, setup, spec, status};
use axum::{
    body::Body,
    http::{Request, StatusCode, header::RETRY_AFTER},
};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn origins() {
    let root = tempfile::tempdir().unwrap();
    let (app, _relay) = setup(
        &root.path().join("data"),
        spec(root.path(), "https://api.locus.example"),
    )
    .await;
    let line = atom("1", PRODUCER, json!({"message": "ok"}));
    let cases = [
        (
            "cross-site",
            Some("https://folio.example"),
            StatusCode::FORBIDDEN,
        ),
        ("same-origin", None, StatusCode::FORBIDDEN),
        (
            "same-origin",
            Some("https://evil.example"),
            StatusCode::FORBIDDEN,
        ),
        (
            "SAME-ORIGIN",
            Some("HTTPS://FOLIO.EXAMPLE:443"),
            StatusCode::NO_CONTENT,
        ),
    ];
    for (site, origin, expected) in cases {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/api/observe")
            .header("sec-fetch-site", site);
        if let Some(origin) = origin {
            builder = builder.header("origin", origin);
        }
        assert_eq!(
            status(&app, builder.body(Body::from(line.clone())).unwrap()).await,
            expected
        );
    }
}

#[tokio::test]
async fn rate() {
    let root = tempfile::tempdir().unwrap();
    let mut cfg = spec(root.path(), "https://api.locus.example");
    cfg.rpm = 60;
    cfg.burst = 2;
    cfg.forwarded = true;
    let (app, _relay) = setup(&root.path().join("data"), cfg).await;
    let line = atom("1", PRODUCER, json!({"message": "ok"}));
    for _ in 0..2 {
        let mut req = postreq("/api/observe", Body::from(line.clone()));
        req.headers_mut()
            .insert("x-forwarded-for", "10.0.0.1, 203.0.113.5".parse().unwrap());
        assert_eq!(status(&app, req).await, StatusCode::NO_CONTENT);
    }
    let mut blocked = postreq("/api/observe", Body::from(line.clone()));
    blocked
        .headers_mut()
        .insert("x-forwarded-for", "10.0.0.9, 203.0.113.5".parse().unwrap());
    let response = app.clone().oneshot(blocked).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    let retry = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|text| text.parse::<u64>().ok());
    assert!(retry.is_some_and(|seconds| seconds >= 1));
    let mut other = postreq("/api/observe", Body::from(line));
    other
        .headers_mut()
        .insert("x-forwarded-for", "10.0.0.1, 203.0.113.6".parse().unwrap());
    assert_eq!(status(&app, other).await, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn encoding() {
    let root = tempfile::tempdir().unwrap();
    let (app, _relay) = setup(
        &root.path().join("data"),
        spec(root.path(), "https://api.locus.example"),
    )
    .await;
    let line = format!("{}\n", atom("1", PRODUCER, json!({"message": "ok"})));
    let mut trailing = gzip(line.as_bytes());
    trailing.extend_from_slice(b"junk");
    let cases = [
        ("gzip", gzip(line.as_bytes()), StatusCode::NO_CONTENT),
        (
            "gzip",
            gzip(&vec![b'a'; 300 * 1024]),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
        ("gzip", b"not-gzip".to_vec(), StatusCode::BAD_REQUEST),
        ("gzip", trailing, StatusCode::BAD_REQUEST),
        ("br", line.into_bytes(), StatusCode::UNSUPPORTED_MEDIA_TYPE),
    ];
    for (enc, bytes, expected) in cases {
        let mut req = postreq("/api/observe", Body::from(bytes));
        req.headers_mut()
            .insert("content-encoding", enc.parse().unwrap());
        assert_eq!(status(&app, req).await, expected);
    }
}

#[tokio::test]
async fn validation() {
    let root = tempfile::tempdir().unwrap();
    let (app, _relay) = setup(
        &root.path().join("data"),
        spec(root.path(), "https://api.locus.example"),
    )
    .await;
    let cases = [
        String::new(),
        atom("1", "other-app", json!({"message": "wrong"})),
        atom("", PRODUCER, json!({"message": "empty-id"})),
        atom("1", PRODUCER, json!({"password": "123"})),
        atom("1", PRODUCER, json!({"nested": [{"access_token": "x"}]})),
        atom("1", PRODUCER, json!({"code_verifier": "v"})),
        atom("1", PRODUCER, json!({"authSecret": "s"})),
        json!({
            "at": 1_u64, "id": "1", "producer": PRODUCER,
            "context": {}, "choices": [], "extra": true
        })
        .to_string(),
        json!({
            "at": 1_u64, "id": "1", "producer": PRODUCER,
            "context": {},
            "choices": [{"role": "token", "key": "k", "origin": "generated"}]
        })
        .to_string(),
    ];
    for payload in cases {
        assert_eq!(
            status(&app, postreq("/api/observe", Body::from(payload))).await,
            StatusCode::BAD_REQUEST
        );
    }
    let many = vec![atom("1", PRODUCER, json!({"message": "ok"})); 513].join("\n");
    assert_eq!(
        status(&app, postreq("/api/observe", Body::from(many))).await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}
