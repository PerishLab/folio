use crate::Patch;
use radicula_document::Value;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Tags {
    pub revision: String,
    pub template: String,
    pub edition: i64,
    pub document: Value,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Amend {
    pub revision: String,
    pub operations: Vec<Patch>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Listed {
    pub id: String,
    pub template: String,
    pub edition: i64,
    pub revision: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Listing {
    pub documents: Vec<Listed>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Entry {
    pub revision: String,
    pub id: String,
    pub template: String,
    pub edition: i64,
    pub document: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub id: String,
    pub template: String,
    pub edition: Option<i64>,
    pub document: Value,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Migrate {
    pub revision: String,
}
