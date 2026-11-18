use crate::{Error, Identity};
use keel_document::{Collection, Shape};
use radicula_document::{Document, Value};
use std::collections::BTreeSet;
use std::sync::Arc;

pub struct Context<'a> {
    pub identity: &'a Identity,
    pub app: Option<&'a str>,
    pub values: &'a Value,
}

#[derive(Clone)]
pub struct Definition {
    pub name: String,
    pub shape: Shape,
    pub default: Value,
    pub options: Vec<Value>,
}

#[derive(Default)]
pub struct Decision {
    pub options: Option<Vec<Value>>,
    pub force: Option<Value>,
    pub readonly: bool,
}

type Policy = Arc<dyn Fn(&Context<'_>, &Definition) -> Decision + Send + Sync>;

pub struct Catalog {
    pub(crate) definitions: Vec<Definition>,
    apps: Vec<String>,
    policy: Policy,
}

impl Definition {
    pub fn validate(&self, value: &Value) -> Result<(), Error> {
        let held = Document::new(value.clone())?;
        Collection::new(&self.name, self.shape.clone())?
            .validate(&held)
            .map_err(|_| Error::Invalid(self.name.clone()))?;
        if !self.options.is_empty() && !self.options.contains(value) {
            return Err(Error::Invalid(self.name.clone()));
        }
        Ok(())
    }
}

impl Catalog {
    pub fn new(definitions: Vec<Definition>, apps: Vec<String>) -> Result<Self, Error> {
        let mut names = BTreeSet::new();
        for definition in &definitions {
            if !name(&definition.name) || !names.insert(definition.name.clone()) {
                return Err(Error::Invalid("definition names".into()));
            }
            definition.validate(&definition.default)?;
            for option in &definition.options {
                definition.validate(option)?;
            }
        }
        names.clear();
        if apps
            .iter()
            .any(|app| !slug(app) || !names.insert(app.clone()))
        {
            return Err(Error::Invalid("application names".into()));
        }
        Ok(Self {
            definitions,
            apps,
            policy: Arc::new(|_, _| Decision::default()),
        })
    }

    pub fn policy(
        mut self,
        policy: impl Fn(&Context<'_>, &Definition) -> Decision + Send + Sync + 'static,
    ) -> Self {
        self.policy = Arc::new(policy);
        self
    }

    pub fn definition(&self, name: &str) -> Result<&Definition, Error> {
        self.definitions
            .iter()
            .find(|definition| definition.name == name)
            .ok_or(Error::Missing)
    }

    pub(crate) fn app(&self, app: Option<&str>) -> Result<(), Error> {
        if app.is_some_and(|app| !self.apps.iter().any(|known| known == app)) {
            return Err(Error::Denied);
        }
        Ok(())
    }

    pub(crate) fn decide(
        &self,
        context: &Context<'_>,
        definition: &Definition,
    ) -> Result<Decision, Error> {
        self.app(context.app)?;
        let decision = (self.policy)(context, definition);
        if let Some(value) = &decision.force {
            definition.validate(value)?;
        }
        if let Some(options) = &decision.options {
            if options.is_empty() {
                return Err(Error::Corrupt);
            }
            for value in options {
                definition.validate(value)?;
            }
        }
        Ok(decision)
    }

    pub fn themes() -> Self {
        let definitions = [
            ("theme.family", "google", vec!["google", "material"]),
            ("theme.tone", "system", vec!["system", "light", "dark"]),
        ]
        .into_iter()
        .map(|(name, default, options)| Definition {
            name: name.into(),
            shape: Shape::Text,
            default: default.into(),
            options: options.into_iter().map(Value::from).collect(),
        })
        .collect();
        Self::new(definitions, vec!["ensign".into()]).expect("static theme declarations")
    }
}

fn slug(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

fn name(value: &str) -> bool {
    value.split('.').all(slug)
}
