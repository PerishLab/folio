use radicula_document::Value;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Template {
    pub name: String,
    pub latest: i64,
    pub editions: Vec<i64>,
    pub builtin: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Templates {
    pub templates: Vec<Template>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Brief {
    pub number: i64,
    pub document: i64,
    pub revision: String,
    #[serde(rename = "schemaVersion")]
    pub version: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Detail {
    pub name: String,
    pub builtin: bool,
    pub latest: i64,
    pub editions: Vec<Brief>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Edition {
    pub template: String,
    pub number: i64,
    pub document: i64,
    pub revision: String,
    #[serde(rename = "schemaVersion")]
    pub version: i64,
    pub schema: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Fault {
    pub id: String,
    pub edition: i64,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Status {
    pub template: String,
    pub latest: i64,
    pub format: String,
    pub ready: bool,
    pub checked: usize,
    pub compatible: usize,
    pub incompatible: Vec<Fault>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Create {
    pub name: String,
    pub schema: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Publish {
    pub base: i64,
    pub schema: Value,
}
