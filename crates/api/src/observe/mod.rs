mod body;
mod check;
mod limit;
mod origin;

use axum::{
    Router,
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderValue, Request, StatusCode, header::RETRY_AFTER},
    response::{IntoResponse, Response},
    routing::post,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

pub struct Spec {
    pub endpoint: String,
    pub key: String,
    pub producer: String,
    pub buffer: PathBuf,
    pub origins: Vec<String>,
    pub rpm: u32,
    pub burst: u32,
    pub forwarded: bool,
}

struct Core {
    queue: locus::relay::Relay,
    limiter: limit::Limiter,
    origins: Vec<String>,
    producer: String,
    forwarded: bool,
}

#[derive(Clone)]
pub struct Relay {
    core: Arc<Core>,
}

impl Relay {
    pub fn open(spec: Spec) -> Result<Self, String> {
        let mut origins = Vec::with_capacity(spec.origins.len());
        for raw in &spec.origins {
            origins.push(origin::parse(raw)?);
        }
        let limiter = limit::Limiter::new(spec.rpm, spec.burst)?;
        let config =
            locus::relay::Config::new(spec.endpoint, spec.key, spec.producer.clone(), spec.buffer);
        let queue = locus::relay::Relay::open(config).map_err(|error| error.to_string())?;
        Ok(Self {
            core: Arc::new(Core {
                queue,
                limiter,
                origins,
                producer: spec.producer,
                forwarded: spec.forwarded,
            }),
        })
    }

    pub fn settle(&self) -> Result<(), String> {
        self.core.queue.settle()
    }
}

pub fn router(relay: Relay) -> Router {
    Router::new().route("/", post(ingest)).with_state(relay)
}

async fn ingest(State(relay): State<Relay>, request: Request<Body>) -> Response {
    let (parts, payload) = request.into_parts();
    if let Err(status) = origin::verify(&parts.headers, &relay.core.origins) {
        return status.into_response();
    }
    let peer = parts
        .extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0);
    let bucket = limit::key(&parts.headers, peer, relay.core.forwarded);
    if let Err(wait) = relay.core.limiter.check(&bucket) {
        return limited(wait);
    }
    let text = match body::read(&parts.headers, payload).await {
        Ok(text) => text,
        Err(status) => return status.into_response(),
    };
    let lines = match check::validate(&text, &relay.core.producer) {
        Ok(lines) => lines,
        Err(status) => return status.into_response(),
    };
    let target = relay.clone();
    match tokio::task::spawn_blocking(move || target.core.queue.enqueue(&lines)).await {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        _ => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

fn limited(wait: u64) -> Response {
    let header =
        HeaderValue::from_str(&wait.to_string()).unwrap_or_else(|_| HeaderValue::from_static("1"));
    (StatusCode::TOO_MANY_REQUESTS, [(RETRY_AFTER, header)]).into_response()
}
