use crate::{Context, Error, Identity, Runtime};
use radicula_document::Value;
use serde::{Serialize, Serializer, ser::SerializeMap};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Light,
    Dark,
}

impl Tone {
    pub fn parse(value: &str) -> Result<Self, Error> {
        match value {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(Error::Invalid("tone".into())),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Device {
    tone: Option<Tone>,
}

impl Device {
    pub fn new(tone: Option<&str>) -> Result<Self, Error> {
        Ok(Self {
            tone: tone.map(Tone::parse).transpose()?,
        })
    }

    pub fn tone(self) -> Option<Tone> {
        self.tone
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Setting,
    Device,
    Default,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Presentation {
    pub system: String,
    pub tone: Tone,
    pub basis: Basis,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct State {
    pub revision: String,
    pub app: Option<String>,
    pub items: Vec<Item>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<Presentation>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct View {
    pub app: Option<String>,
    #[serde(serialize_with = "named")]
    pub items: Vec<Item>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<Presentation>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Views {
    pub shared: View,
    pub app: Option<View>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    pub revision: String,
    pub document: Views,
}

impl View {
    pub(crate) fn state(self, revision: String) -> State {
        State {
            revision,
            app: self.app,
            items: self.items,
            presentation: self.presentation,
        }
    }
}

impl Runtime {
    pub(crate) fn views(
        &self,
        identity: &Identity,
        app: Option<&str>,
        body: &Value,
        device: crate::Device,
    ) -> Result<Views, Error> {
        let shared = self.evaluate(identity, None, body, device)?;
        let app = app
            .map(|name| self.evaluate(identity, Some(name), body, device))
            .transpose()?;
        Ok(Views { shared, app })
    }

    pub(crate) fn evaluate(
        &self,
        identity: &Identity,
        app: Option<&str>,
        body: &Value,
        device: Device,
    ) -> Result<View, Error> {
        let context = Context {
            identity,
            app,
            values: body,
        };
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
        let presentation = if self.catalog.presentation {
            present(&items, device)?
        } else {
            None
        };
        Ok(View {
            app: context.app.map(str::to_string),
            items,
            presentation,
        })
    }
}

fn named<S: Serializer>(items: &[Item], serializer: S) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(items.len()))?;
    for item in items {
        map.serialize_entry(&item.name, item)?;
    }
    map.end()
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

fn present(items: &[Item], device: Device) -> Result<Option<Presentation>, Error> {
    let family = items
        .iter()
        .find(|item| item.name == "theme.family")
        .and_then(|item| item.effective.as_str());
    let preference = items
        .iter()
        .find(|item| item.name == "theme.tone")
        .and_then(|item| item.effective.as_str());
    let (Some(family), Some(preference)) = (family, preference) else {
        return Ok(None);
    };
    let system = system(family);
    let (tone, basis) = match preference {
        "light" => (Tone::Light, Basis::Setting),
        "dark" => (Tone::Dark, Basis::Setting),
        "system" => match device.tone() {
            Some(tone) => (tone, Basis::Device),
            None => (Tone::Light, Basis::Default),
        },
        _ => return Err(Error::Corrupt),
    };
    Ok(Some(Presentation {
        system: system.into(),
        tone,
        basis,
    }))
}

fn system(family: &str) -> &str {
    match family {
        "google" => "google-accounts",
        family => family,
    }
}

fn declared(value: Option<&Value>) -> Declared {
    value
        .cloned()
        .map(Declared::Present)
        .unwrap_or(Declared::Missing)
}
