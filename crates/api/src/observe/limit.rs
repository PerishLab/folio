use axum::http::HeaderMap;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const LIMIT: usize = 4096;

struct Bucket {
    tokens: f64,
    updated: Instant,
}

pub(super) struct Limiter {
    rate: f64,
    capacity: f64,
    stale: Duration,
    state: Mutex<HashMap<String, Bucket>>,
}

impl Limiter {
    pub(super) fn new(rpm: u32, burst: u32) -> Result<Self, String> {
        if rpm == 0 || burst == 0 {
            return Err("locus rpm and burst must be positive".into());
        }
        let rate = f64::from(rpm) / 60.0;
        let capacity = f64::from(burst);
        let stale = Duration::from_secs_f64((capacity / rate).max(60.0));
        Ok(Self {
            rate,
            capacity,
            stale,
            state: Mutex::new(HashMap::new()),
        })
    }

    pub(super) fn check(&self, key: &str) -> Result<(), u64> {
        let now = Instant::now();
        let mut map = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if !map.contains_key(key) && map.len() >= LIMIT {
            evict(&mut map, now, self.stale);
        }
        let entry = map.entry(key.to_owned()).or_insert_with(|| Bucket {
            tokens: self.capacity,
            updated: now,
        });
        let elapsed = now.saturating_duration_since(entry.updated).as_secs_f64();
        entry.tokens = (entry.tokens + elapsed * self.rate).min(self.capacity);
        entry.updated = now;
        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            return Ok(());
        }
        let needed = (1.0 - entry.tokens) / self.rate;
        Err(needed.ceil().max(1.0) as u64)
    }
}

fn evict(map: &mut HashMap<String, Bucket>, now: Instant, stale: Duration) {
    map.retain(|_, bucket| now.saturating_duration_since(bucket.updated) < stale);
    if map.len() < LIMIT {
        return;
    }
    let oldest = map
        .iter()
        .min_by_key(|(_, bucket)| bucket.updated)
        .map(|(key, _)| key.clone());
    if let Some(key) = oldest {
        map.remove(&key);
    }
}

pub(super) fn key(headers: &HeaderMap, peer: Option<SocketAddr>, forwarded: bool) -> String {
    if forwarded && let Some(ip) = client(headers) {
        return ip.to_string();
    }
    peer.map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "default".into())
}

fn client(headers: &HeaderMap) -> Option<IpAddr> {
    let direct = headers
        .get_all("x-real-ip")
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok())
        .and_then(|text| text.trim().parse::<IpAddr>().ok());
    if direct.is_some() {
        return direct;
    }
    headers
        .get_all("x-forwarded-for")
        .iter()
        .next_back()
        .and_then(|value| value.to_str().ok())
        .and_then(|text| text.rsplit(',').next())
        .and_then(|entry| entry.trim().parse::<IpAddr>().ok())
}
