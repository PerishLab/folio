use axum::http::{HeaderMap, StatusCode};
use url::Url;

pub(super) fn parse(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    let url = Url::parse(trimmed).map_err(|_| format!("invalid origin: {trimmed}"))?;
    if !scheme(&url) || !host(&url) || !bare(&url) {
        return Err(format!("invalid origin: {trimmed}"));
    }
    Ok(url.origin().ascii_serialization())
}

fn scheme(url: &Url) -> bool {
    matches!(url.scheme(), "http" | "https")
}

fn host(url: &Url) -> bool {
    url.has_host() && url.username().is_empty() && url.password().is_none()
}

fn bare(url: &Url) -> bool {
    url.path() == "/" && url.query().is_none() && url.fragment().is_none()
}

pub(super) fn verify(headers: &HeaderMap, allowed: &[String]) -> Result<(), StatusCode> {
    let mut fetched = headers.get_all("sec-fetch-site").iter();
    if let Some(first) = fetched.next() {
        if fetched.next().is_some() {
            return Err(StatusCode::FORBIDDEN);
        }
        let text = first.to_str().map_err(|_| StatusCode::FORBIDDEN)?;
        if !text.trim().eq_ignore_ascii_case("same-origin") {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    if allowed.is_empty() {
        return Ok(());
    }
    let mut origins = headers.get_all("origin").iter();
    let header = origins.next().ok_or(StatusCode::FORBIDDEN)?;
    if origins.next().is_some() {
        return Err(StatusCode::FORBIDDEN);
    }
    let text = header.to_str().map_err(|_| StatusCode::FORBIDDEN)?;
    let canonical = parse(text).map_err(|_| StatusCode::FORBIDDEN)?;
    if !allowed.iter().any(|item| item == &canonical) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}
