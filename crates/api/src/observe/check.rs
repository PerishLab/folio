use axum::http::StatusCode;
use locus::Atom;
use serde_json::Value;

const LINES: usize = 512;
const FIELDS: [&str; 8] = [
    "at",
    "id",
    "producer",
    "context",
    "choices",
    "collections",
    "source",
    "payload",
];
const BANNED: [&str; 9] = [
    "password",
    "pass",
    "token",
    "secret",
    "authorization",
    "cookie",
    "verifier",
    "rescue",
    "sudo",
];

pub(super) fn validate(text: &str, producer: &str) -> Result<Vec<String>, StatusCode> {
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    if trimmed.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut lines = Vec::new();
    for raw in trimmed.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim().is_empty() {
            return Err(StatusCode::BAD_REQUEST);
        }
        if lines.len() >= LINES {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        record(line, producer)?;
        lines.push(line.to_owned());
    }
    Ok(lines)
}

fn record(line: &str, producer: &str) -> Result<(), StatusCode> {
    let value: Value = serde_json::from_str(line).map_err(|_| StatusCode::BAD_REQUEST)?;
    let object = value.as_object().ok_or(StatusCode::BAD_REQUEST)?;
    if object.keys().any(|key| !FIELDS.contains(&key.as_str())) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if dirty(&value) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let atom: Atom = serde_json::from_value(value).map_err(|_| StatusCode::BAD_REQUEST)?;
    let named = atom.producer() == Some(producer);
    let identified = atom.id().is_some_and(|id| !id.trim().is_empty());
    if !named || !identified {
        return Err(StatusCode::BAD_REQUEST);
    }
    if atom.choices().iter().any(|choice| sensitive(choice.role())) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if atom
        .collections()
        .iter()
        .any(|collection| sensitive(collection.role()))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn dirty(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, child)| sensitive(key) || dirty(child)),
        Value::Array(items) => items.iter().any(dirty),
        _ => false,
    }
}

fn sensitive(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    if lower == "code_verifier" || lower.contains("code_verifier") {
        return true;
    }
    segments(key).into_iter().any(|segment| {
        let word = segment.to_ascii_lowercase();
        BANNED.contains(&word.as_str())
    })
}

fn segments(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut previous = None::<char>;
    for ch in text.chars() {
        if !ch.is_ascii_alphanumeric() {
            flush(&mut out, &mut current);
            previous = None;
            continue;
        }
        if boundary(previous, ch) {
            flush(&mut out, &mut current);
        }
        current.push(ch);
        previous = Some(ch);
    }
    flush(&mut out, &mut current);
    out
}

fn boundary(previous: Option<char>, current: char) -> bool {
    previous.is_some_and(|prior| prior.is_ascii_lowercase() && current.is_ascii_uppercase())
}

fn flush(out: &mut Vec<String>, current: &mut String) {
    if !current.is_empty() {
        out.push(std::mem::take(current));
    }
}
