use crate::{Catalog, Error};
use keel::{Core, Graph, Op, Row, adapt::db::Sqlite, atom::Kind, bond, form, spec::Spec};
use keel_document::Store;
use radicula_document::{Document, Value};
use serde_json::{Map, json};

pub(crate) const COLLECTION: &str = "schemas";
const DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";
const EDITION: &str = "template:edition";
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

fn bodies() -> Result<[(&'static str, Document); 2], Error> {
    let meta = meta();
    let check = jsonschema::draft202012::new(&meta).map_err(|_| Error::Corrupt)?;
    let meta = json!({"schemaVersion": 1, "schema": meta});
    let theme = json!({"schemaVersion": 1, "schema": theme()});
    if !check.is_valid(&meta) || !check.is_valid(&theme) {
        return Err(Error::Corrupt);
    }
    Ok([
        ("meta", Document::new(meta)?),
        ("theme", Document::new(theme)?),
    ])
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
