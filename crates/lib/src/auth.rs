use crate::Error;
use radicula_oidc::jwt::{Algorithm, DecodingKey, Rules, Verifier};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct Identity {
    issuer: String,
    subject: String,
    expires: u64,
}

pub struct Auth {
    issuer: String,
    audience: String,
    keys: Vec<Verifier>,
}

impl Identity {
    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub(crate) fn current(&self, issuer: &str) -> Result<(), Error> {
        if self.issuer != issuer || crate::clock()? >= self.expires {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }
}

impl Auth {
    pub fn new(issuer: String, audience: String, keys: &Value) -> Result<Self, Error> {
        if issuer.is_empty() || audience.is_empty() {
            return Err(Error::Unauthorized);
        }
        let entries = keys
            .get("keys")
            .and_then(Value::as_array)
            .ok_or(Error::Unauthorized)?;
        let mut seen = BTreeSet::new();
        let mut trusted = Vec::new();
        for entry in entries {
            let key = verifier(entry)?;
            if !seen.insert(key.kid.clone()) {
                return Err(Error::Unauthorized);
            }
            trusted.push(key);
        }
        if trusted.is_empty() {
            return Err(Error::Unauthorized);
        }
        Ok(Self {
            issuer,
            audience,
            keys: trusted,
        })
    }

    pub fn verify(&self, token: &str) -> Result<Identity, Error> {
        let rules = Rules {
            issuer: &self.issuer,
            audience: &self.audience,
            subject: None,
            now: crate::clock()?,
            leeway: 0,
            required: &[("kind", "access")],
        };
        for verifier in &self.keys {
            if let Ok(claims) = verifier.verify(token, &rules) {
                return Ok(Identity {
                    issuer: claims.iss,
                    subject: claims.sub,
                    expires: claims.exp,
                });
            }
        }
        Err(Error::Unauthorized)
    }
}

fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str, Error> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|held| !held.is_empty())
        .ok_or(Error::Unauthorized)
}

fn verifier(value: &Value) -> Result<Verifier, Error> {
    if text(value, "kty")? != "EC"
        || text(value, "crv")? != "P-256"
        || text(value, "alg")? != "ES256"
        || text(value, "use")? != "sig"
    {
        return Err(Error::Unauthorized);
    }
    let key = DecodingKey::from_ec_components(text(value, "x")?, text(value, "y")?)
        .map_err(|_| Error::Unauthorized)?;
    Ok(Verifier {
        algorithm: Algorithm::ES256,
        kid: text(value, "kid")?.into(),
        key,
    })
}
