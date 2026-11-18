use radicula_config::Cascade;
use std::path::{Path, PathBuf};

#[derive(Debug, Cascade)]
#[plumb::config::prefix]
pub struct Settings {
    pub issuer: String,
    pub audience: String,
    pub keys: String,
    pub data: String,
    pub listen: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            issuer: String::new(),
            audience: String::new(),
            keys: "jwks.json".into(),
            data: "data".into(),
            listen: "127.0.0.1:8080".into(),
        }
    }
}

impl Settings {
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join("folio.toml");
        let settings = Self::resolve(Some(&path), &Self::prefix(), &plumb::config::value)
            .map_err(|error| error.to_string())?;
        if settings.issuer.is_empty() || settings.audience.is_empty() {
            return Err("issuer and audience are required".into());
        }
        Ok(settings)
    }

    pub fn data(&self, root: &Path) -> PathBuf {
        plumb::config::rebase(Path::new(&self.data), root)
    }

    pub fn auth(&self, root: &Path) -> Result<folio::Auth, String> {
        let path = plumb::config::rebase(Path::new(&self.keys), root);
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let keys = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
        folio::Auth::new(self.issuer.clone(), self.audience.clone(), &keys)
            .map_err(|error| error.to_string())
    }
}
