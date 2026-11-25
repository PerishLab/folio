use crate::{Catalog, Context, Error};
use radicula_document::{Document, Operation, Path, Presence, Value};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "kind",
    content = "name",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Scope {
    Shared,
    App(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Patch {
    Set { path: String, value: Value },
    Remove { path: String },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Set {
        scope: Scope,
        name: String,
        value: Value,
    },
    Reset {
        scope: Scope,
        name: String,
    },
    Patch {
        scope: Scope,
        name: String,
        operations: Vec<Patch>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Write {
    pub revision: String,
    pub changes: Vec<Change>,
}

impl Scope {
    pub(crate) fn app(&self) -> Option<&str> {
        match self {
            Self::Shared => None,
            Self::App(name) => Some(name),
        }
    }

    fn prefix(&self) -> String {
        match self {
            Self::Shared => "/shared".into(),
            Self::App(name) => format!("/apps/{name}"),
        }
    }
}

impl Change {
    fn target(&self) -> (&Scope, &str) {
        match self {
            Self::Set { scope, name, .. }
            | Self::Reset { scope, name }
            | Self::Patch { scope, name, .. } => (scope, name),
        }
    }

    pub(crate) fn apply(
        &self,
        catalog: &Catalog,
        identity: &crate::Identity,
        body: &mut Document,
    ) -> Result<(), Error> {
        let (scope, name) = self.target();
        let definition = catalog.definition(name)?;
        let context = Context {
            identity,
            app: scope.app(),
            values: body.root(),
        };
        let decision = catalog.decide(&context, definition)?;
        if decision.readonly || decision.force.is_some() {
            return Err(Error::Denied);
        }
        let prefix = Path::parse(&scope.prefix())?;
        if body.read(&prefix)? == Presence::Missing {
            body.set(&prefix, serde_json::json!({}))?;
        }
        let path = Path::parse(&format!("{}/{name}", scope.prefix()))?;
        let value = self.value(body.read(&path)?.value())?;
        match value {
            Some(value) => {
                definition.validate(&value)?;
                if decision
                    .options
                    .as_ref()
                    .is_some_and(|options| !options.contains(&value))
                {
                    return Err(Error::Denied);
                }
                body.set(&path, value)?;
            }
            None if body.read(&path)? != Presence::Missing => body.remove(&path)?,
            None => {}
        }
        Ok(())
    }

    fn value(&self, current: Option<&Value>) -> Result<Option<Value>, Error> {
        match self {
            Self::Set { value, .. } => Ok(Some(value.clone())),
            Self::Reset { .. } => Ok(None),
            Self::Patch { operations, .. } => {
                let mut held = Document::new(current.cloned().ok_or(Error::Missing)?)?;
                let operations = operations
                    .iter()
                    .map(Patch::operation)
                    .collect::<Result<Vec<_>, _>>()?;
                held.apply(&operations).map_err(Error::Patch)?;
                Ok(Some(held.take()))
            }
        }
    }
}

impl Patch {
    pub(crate) fn operation(&self) -> Result<Operation, Error> {
        Ok(match self {
            Self::Set { path, value } => Operation::Set {
                path: Path::parse(path)?,
                value: value.clone(),
            },
            Self::Remove { path } => Operation::Remove {
                path: Path::parse(path)?,
            },
        })
    }
}
