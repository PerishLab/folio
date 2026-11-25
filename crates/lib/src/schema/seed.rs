use crate::{Catalog, Error};
use jsonschema::Validator;
use keel::{Core, Graph, Op, Row, adapt::db::Sqlite, atom::Kind, bond, form, spec::Spec};
use keel_document::Store;
use radicula_document::{Document, Value};
use serde_json::{Map, json};

pub(crate) const COLLECTION: &str = "schemas";
pub(crate) const EDITION: &str = "template:edition";
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const NUMBER: &str = "1";

pub(crate) fn stock(graph: &mut Graph) {
    graph.add(
        Spec::build("Template")
            .sole("name", Kind::Text)
            .freeze()
            .seal(),
    );
    graph.add(
        Spec::build("Edition")
            .root("template", bond::Kind::Many2one, "Template")
            .per("number", Kind::Int, &["template"])
            .bond(
                "document",
                bond::Kind::Many2one,
                "documentstore:document",
                &[],
            )
            .freeze()
            .seal(),
    );
}

pub(crate) fn builtin(name: &str) -> bool {
    matches!(name, "meta" | "theme" | "tags")
}

fn meta() -> Value {
    json!({
        "$schema": DIALECT,
        "type": "object",
        "properties": {
            "schemaVersion": {"const": 1},
            "schema": {"$ref": DIALECT}
        },
        "required": ["schemaVersion", "schema"],
        "additionalProperties": false
    })
}

fn theme() -> Value {
    let props: Map<_, _> = Catalog::themes()
        .definitions
        .into_iter()
        .map(|item| {
            let rules = json!({"type": "string", "default": item.default, "enum": item.options});
            (item.name, rules)
        })
        .collect();
    json!({
        "$schema": DIALECT,
        "type": "object",
        "properties": props,
        "additionalProperties": false
    })
}

fn tags() -> Value {
    let item = json!({
        "type": "object",
        "properties": {
            "label": {"type": "string", "minLength": 1},
            "color": {"type": "string"}
        },
        "required": ["label"],
        "additionalProperties": false
    });
    let map = json!({"type": "object", "additionalProperties": item});
    json!({
        "$schema": DIALECT,
        "type": "object",
        "properties": {"tags": map},
        "required": ["tags"],
        "additionalProperties": false
    })
}

pub(crate) fn compile(schema: &Value) -> Result<Validator, Error> {
    jsonschema::draft202012::new(schema).map_err(|_| Error::Invalid("schema".into()))
}

pub(crate) fn envelope(schema: &Value) -> Result<Document, Error> {
    let check = jsonschema::draft202012::new(&meta()).map_err(|_| Error::Corrupt)?;
    let wrapped = json!({"schemaVersion": 1, "schema": schema});
    if !check.is_valid(&wrapped) {
        return Err(Error::Invalid("schema".into()));
    }
    compile(schema)?;
    Document::new(wrapped).map_err(Into::into)
}

pub(crate) fn unwrap(body: &Value) -> Result<(i64, Value), Error> {
    let version = body
        .get("schemaVersion")
        .and_then(Value::as_i64)
        .ok_or(Error::Corrupt)?;
    let schema = body.get("schema").cloned().ok_or(Error::Corrupt)?;
    Ok((version, schema))
}

fn bodies() -> Result<[(&'static str, Document); 3], Error> {
    let root = meta();
    let check = jsonschema::draft202012::new(&root).map_err(|_| Error::Corrupt)?;
    let items = [("meta", root), ("theme", theme()), ("tags", tags())];
    let mut built = Vec::with_capacity(3);
    for (name, schema) in items {
        let wrapped = json!({"schemaVersion": 1, "schema": schema});
        if !check.is_valid(&wrapped) || jsonschema::draft202012::new(&schema).is_err() {
            return Err(Error::Corrupt);
        }
        built.push((name, Document::new(wrapped)?));
    }
    built.try_into().map_err(|_| Error::Corrupt)
}

pub(crate) async fn seed(core: &Core<Sqlite>, store: &Store<Sqlite>) -> Result<(), Error> {
    for (name, body) in bodies()? {
        let document = store.sudo().create(COLLECTION, name, body).await?;
        let template = core.put("Template", &[("name", name)]).await?;
        core.put(
            EDITION,
            &[
                ("template", &template.to_string()),
                ("number", NUMBER),
                ("document", &document.head.id.to_string()),
            ],
        )
        .await?;
    }
    verify(core, store).await
}

async fn edition(core: &Core<Sqlite>, name: &str) -> Result<Row, Error> {
    let template = core
        .one(&form("Template").when("name", Op::Eq, name))
        .await?
        .ok_or(Error::Corrupt)?;
    let query = form(EDITION)
        .when("template", Op::Eq, &template.key().to_string())
        .when("number", Op::Eq, NUMBER);
    core.one(&query).await?.ok_or(Error::Corrupt)
}

pub(crate) async fn verify(core: &Core<Sqlite>, store: &Store<Sqlite>) -> Result<(), Error> {
    for (name, body) in bodies()? {
        let record = edition(core, name).await?;
        let id = record.int("document").ok_or(Error::Corrupt)?;
        let held = store.sudo().read(COLLECTION, id).await?;
        if held.value != body {
            return Err(Error::Corrupt);
        }
    }
    Ok(())
}
