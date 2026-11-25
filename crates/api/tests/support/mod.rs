#![allow(dead_code)]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use folio::{Auth, Catalog, Runtime};
use p256::{
    elliptic_curve::sec1::ToEncodedPoint,
    pkcs8::{EncodePrivateKey, LineEnding},
};
use radicula_oidc::jwt::{Algorithm, Audience, Claims, EncodingKey, Signer};
use serde_json::{Value, json};
use tower::ServiceExt;

pub const ISSUER: &str = "https://identity.test/api";
pub const AUDIENCE: &str = "https://folio.test";

pub struct Keys {
    pub signer: Signer,
    pub jwks: Value,
}

impl Keys {
    pub fn new() -> Self {
        let key = p256::SecretKey::from_slice(&[7; 32]).unwrap();
        let public = key.public_key().to_encoded_point(false);
        let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap();
        let signer = Signer {
            algorithm: Algorithm::ES256,
            kid: "fixture".into(),
            key: EncodingKey::from_ec_pem(pem.as_bytes()).unwrap(),
        };
        let jwks = json!({"keys":[{"kty":"EC","crv":"P-256","alg":"ES256","use":"sig","kid":"fixture",
            "x":URL_SAFE_NO_PAD.encode(public.x().unwrap()), "y":URL_SAFE_NO_PAD.encode(public.y().unwrap())}]});
        Self { signer, jwks }
    }

    pub fn auth(&self) -> Auth {
        Auth::new(ISSUER.into(), AUDIENCE.into(), &self.jwks).unwrap()
    }

    pub fn claims(&self, subject: &str) -> Claims {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        Claims {
            iss: ISSUER.into(),
            sub: subject.into(),
            aud: Audience::One(AUDIENCE.into()),
            iat: now - 10,
            exp: now + 600,
            nbf: None,
            extra: serde_json::Map::from_iter([("kind".into(), json!("access"))]),
        }
    }

    pub fn token(&self, subject: &str) -> String {
        self.signer
            .sign(&self.claims(subject))
            .unwrap()
            .expose()
            .into()
    }
}

pub async fn router(root: &std::path::Path, catalog: Catalog, keys: &Keys) -> Router {
    let runtime = Runtime::create(root, catalog, ISSUER.into()).await.unwrap();
    api::router(runtime, keys.auth())
}

pub fn request(method: &str, path: &str, token: &str, body: Value) -> Request<Body> {
    let payload = if body.is_null() && matches!(method, "GET" | "DELETE") {
        Body::empty()
    } else {
        Body::from(body.to_string())
    };
    Request::builder()
        .method(method)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(payload)
        .unwrap()
}

pub async fn call(router: &Router, ask: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(ask).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&body).unwrap_or_else(|_| json!(String::from_utf8_lossy(&body))),
    )
}

pub async fn shown(router: &Router, token: &str, query: &str) -> Value {
    let path = format!("/v1/me/settings?app=ensign{query}");
    let (status, body) = call(router, request("GET", &path, token, Value::Null)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

pub async fn read(router: &Router, token: &str) -> Value {
    let (status, body) = call(
        router,
        request("GET", "/v1/me/settings?app=ensign", token, Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

pub fn item<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
        .unwrap()
}

pub fn set(scope: Value, name: &str, value: Value) -> Value {
    json!({"op":"set","scope":scope,"name":name,"value":value})
}
pub fn shared() -> Value {
    json!({"kind":"shared"})
}
pub fn app() -> Value {
    json!({"kind":"app","name":"ensign"})
}

pub async fn grant(root: &std::path::Path, subject: &str) {
    let mut graph = keel::Graph::new();
    folio::stock(&mut graph);
    let wire = keel::adapt::db::Sqlite::file(&root.join("estate.sqlite3"))
        .await
        .unwrap();
    let core = keel::bind(graph, wire)
        .await
        .unwrap()
        .identify("Actor")
        .unwrap();
    let query = keel::form("Actor")
        .when("issuer", keel::Op::Eq, ISSUER)
        .when("subject", keel::Op::Eq, subject);
    let actor = core.one(&query).await.unwrap().unwrap().key().to_string();
    let scopes = [
        ("Template", "all"),
        ("template:edition", "all"),
        ("documentstore:document", "pred collection = \"schemas\""),
    ];
    for (unit, scope) in scopes {
        core.put(
            "@grant",
            &[
                ("who", actor.as_str()),
                ("verb", "put"),
                ("unit", unit),
                ("scope", scope),
            ],
        )
        .await
        .unwrap();
    }
}
