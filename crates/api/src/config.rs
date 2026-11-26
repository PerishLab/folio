use radicula_config::Cascade;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Cascade)]
#[cascade(section)]
pub struct Locus {
    pub endpoint: String,
    pub key: String,
    pub keyfile: String,
    pub producer: String,
    pub buffer: String,
    pub origins: String,
    pub rpm: u32,
    pub burst: u32,
    pub forwarded: bool,
}

impl Default for Locus {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            key: String::new(),
            keyfile: String::new(),
            producer: "folio-web".into(),
            buffer: "locus-relay".into(),
            origins: String::new(),
            rpm: 120,
            burst: 30,
            forwarded: false,
        }
    }
}

impl fmt::Debug for Locus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Locus")
            .field("endpoint", &self.endpoint)
            .field("key", &"[redacted]")
            .field("keyfile", &self.keyfile)
            .field("producer", &self.producer)
            .field("buffer", &self.buffer)
            .field("origins", &self.origins)
            .field("rpm", &self.rpm)
            .field("burst", &self.burst)
            .field("forwarded", &self.forwarded)
            .finish()
    }
}

#[derive(Debug, Cascade)]
#[plumb::config::prefix("FOLIO")]
pub struct Settings {
    pub issuer: String,
    pub audience: String,
    pub keys: String,
    pub data: String,
    pub listen: String,
    pub apps: Vec<String>,
    #[cascade(section)]
    pub locus: Locus,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            audience: String::new(),
            keys: "jwks.json".into(),
            data: "data".into(),
            listen: "127.0.0.1:8080".into(),
            apps: Vec::new(),
            locus: Locus::default(),
        }
    }
}

impl Settings {
    pub fn load(root: &Path) -> Result<Self, String> {
        Self::from(root, &plumb::config::value)
    }

    pub fn from(root: &Path, get: &dyn Fn(&str) -> Option<String>) -> Result<Self, String> {
        let env = plumb::config::migrated(&Self::prefix(), "API", get);
        let path = root.join("folio.toml");
        let mut settings =
            Self::resolve(Some(&path), &Self::prefix(), &env).map_err(|error| error.to_string())?;
        if let Some(raw) = env(&format!("{}_APPS", Self::prefix())) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                settings.apps = trimmed
                    .split(',')
                    .map(|part| part.trim().to_string())
                    .collect();
            }
        }
        if let Some(raw) = env("SIDECAR_PORT") {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                let port: u16 = trimmed
                    .parse()
                    .map_err(|_| "SIDECAR_PORT must be a valid port".to_string())?;
                settings.listen = bind(&settings.listen, port);
            }
        }
        if settings.issuer.is_empty() || settings.audience.is_empty() {
            return Err("issuer and audience are required".into());
        }
        Ok(settings)
    }

    pub fn data(&self, root: &Path) -> PathBuf {
        plumb::config::rebase(Path::new(&self.data), root)
    }

    pub fn catalog(&self) -> Result<folio::Catalog, String> {
        folio::Catalog::themes()
            .extend(self.apps.clone())
            .map_err(|error| error.to_string())
    }

    pub fn auth(&self, root: &Path) -> Result<folio::Auth, String> {
        let path = plumb::config::rebase(Path::new(&self.keys), root);
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let keys = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        folio::Auth::new(self.issuer.clone(), self.audience.clone(), &keys)
            .map_err(|error| error.to_string())
    }

    pub fn relay(&self, root: &Path) -> Result<Option<locus_relay::Relay>, String> {
        let endpoint = self.locus.endpoint.trim();
        if endpoint.is_empty() {
            return Ok(None);
        }
        let key = secret(&self.locus, root)?;
        let buffer = absolute(plumb::config::rebase(Path::new(&self.locus.buffer), root))?;
        let origins = self
            .locus
            .origins
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_owned)
            .collect();
        let config = locus_relay::Config::new(
            endpoint.to_owned(),
            key,
            self.locus.producer.trim().to_owned(),
            buffer,
        )
        .origins(origins)
        .rpm(self.locus.rpm)
        .burst(self.locus.burst)
        .forwarded(self.locus.forwarded);
        locus_relay::Relay::open(config)
            .map(Some)
            .map_err(|error| error.to_string())
    }
}

fn secret(locus: &Locus, root: &Path) -> Result<String, String> {
    let direct = locus.key.trim();
    if !direct.is_empty() {
        return Ok(direct.to_owned());
    }
    let file = locus.keyfile.trim();
    if file.is_empty() {
        return Err("locus key or keyfile is required".into());
    }
    let path = plumb::config::rebase(Path::new(file), root);
    locus::delivery::transport::secret(&path)
}

fn absolute(path: PathBuf) -> Result<PathBuf, String> {
    std::path::absolute(path).map_err(|error| error.to_string())
}

fn bind(listen: &str, port: u16) -> String {
    match listen.rsplit_once(':') {
        Some((host, _)) if !host.is_empty() => format!("{host}:{port}"),
        _ => format!("127.0.0.1:{port}"),
    }
}
