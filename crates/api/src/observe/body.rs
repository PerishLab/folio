use axum::{
    body::{Body, to_bytes},
    http::{HeaderMap, StatusCode},
};
use flate2::read::MultiGzDecoder;
use std::io::{Cursor, Read};

const PLAIN: usize = 256 * 1024;
const COMPRESSED: usize = 128 * 1024;

enum Encoding {
    Identity,
    Gzip,
}

pub(super) async fn read(headers: &HeaderMap, body: Body) -> Result<String, StatusCode> {
    let encoding = encoding(headers)?;
    let bytes = match encoding {
        Encoding::Identity => raw(body, PLAIN).await?,
        Encoding::Gzip => decompress(&raw(body, COMPRESSED).await?)?,
    };
    String::from_utf8(bytes).map_err(|_| StatusCode::BAD_REQUEST)
}

fn encoding(headers: &HeaderMap) -> Result<Encoding, StatusCode> {
    let mut values = headers.get_all("content-encoding").iter();
    let Some(first) = values.next() else {
        return Ok(Encoding::Identity);
    };
    if values.next().is_some() {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let text = first
        .to_str()
        .map_err(|_| StatusCode::UNSUPPORTED_MEDIA_TYPE)?;
    let lower = text.trim().to_ascii_lowercase();
    match lower.as_str() {
        "" | "identity" => Ok(Encoding::Identity),
        "gzip" => Ok(Encoding::Gzip),
        _ => Err(StatusCode::UNSUPPORTED_MEDIA_TYPE),
    }
}

async fn raw(body: Body, limit: usize) -> Result<Vec<u8>, StatusCode> {
    let bytes = to_bytes(body, limit)
        .await
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
    Ok(bytes.to_vec())
}

fn decompress(compressed: &[u8]) -> Result<Vec<u8>, StatusCode> {
    if compressed.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut decoder = MultiGzDecoder::new(Cursor::new(compressed)).take((PLAIN + 1) as u64);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    if out.len() > PLAIN {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let cursor = decoder.into_inner().into_inner();
    if cursor.position() as usize != compressed.len() {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(out)
}
