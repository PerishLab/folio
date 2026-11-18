use crate::{Context, Error, Identity, Runtime};
use keel_document::Record;
use radicula_document::Value;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "state", content = "value", rename_all = "snake_case")]
pub enum Declared {
    Missing,
    Present(Value),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Default,
    Shared,
    App,
    Policy,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Item {
    pub name: String,
    pub options: Vec<Value>,
    pub shared: Declared,
    pub app: Declared,
    pub effective: Value,
    pub source: Source,
    pub editable: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub revision: String,
    pub app: Option<String>,
    pub items: Vec<Item>,
}

impl Runtime {
    pub(crate) fn evaluate(
        &self,
        identity: &Identity,
        app: Option<&str>,
        record: &Record,
    ) -> Result<State, Error> {
        let context = Context {
            identity,
            app,
            values: record.value.root(),
        };
        let body = record.value.root();
        let shared = body
            .get("shared")
            .and_then(Value::as_object)
            .ok_or(Error::Corrupt)?;
        let apps = body
            .get("apps")
            .and_then(Value::as_object)
            .ok_or(Error::Corrupt)?;
        let app = context
            .app
            .and_then(|name| apps.get(name))
            .and_then(Value::as_object);
        let mut items = Vec::new();
        for definition in &self.catalog.definitions {
            let decision = self.catalog.decide(&context, definition)?;
            let base = shared.get(&definition.name);
            let overlay = app.and_then(|values| values.get(&definition.name));
            let (effective, source) = selected(definition, base, overlay, &decision);
            let effective = effective.clone();
            items.push(Item {
                name: definition.name.clone(),
                options: decision
                    .options
                    .unwrap_or_else(|| definition.options.clone()),
                shared: declared(base),
                app: declared(overlay),
                effective,
                source,
                editable: !decision.readonly && decision.force.is_none(),
            });
        }
        Ok(State {
            revision: record.head.token.clone(),
            app: context.app.map(str::to_string),
            items,
        })
    }
}

fn selected<'a>(
    definition: &'a crate::Definition,
    shared: Option<&'a Value>,
    app: Option<&'a Value>,
    policy: &'a crate::Decision,
) -> (&'a Value, Source) {
    if let Some(value) = &policy.force {
        return (value, Source::Policy);
    }
    let candidates = [
        (app, Source::App),
        (shared, Source::Shared),
        (Some(&definition.default), Source::Default),
    ];
    for (value, source) in candidates {
        if let Some(value) = value
            && definition.validate(value).is_ok()
            && policy
                .options
                .as_ref()
                .is_none_or(|options| options.contains(value))
        {
            return (value, source);
        }
    }
    let value = policy
        .options
        .as_ref()
        .and_then(|options| options.first())
        .expect("validated policy choices");
    (value, Source::Policy)
}

fn declared(value: Option<&Value>) -> Declared {
    value
        .cloned()
        .map(Declared::Present)
        .unwrap_or(Declared::Missing)
}
