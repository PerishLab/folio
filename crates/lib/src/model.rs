use crate::{Error, entry, schema};
use keel::{Graph, atom::Kind, spec::Spec};
use keel_document::{Collection, Field, Model, Shape};

struct Grant {
    who: &'static str,
    verb: &'static str,
    unit: &'static str,
    scope: &'static str,
}

const GRANTS: &[Grant] = &[
    Grant {
        who: "anon",
        verb: "put",
        unit: "Actor",
        scope: "all",
    },
    Grant {
        who: "anon",
        verb: "see",
        unit: "Actor",
        scope: "all",
    },
    Grant {
        who: "all",
        verb: "put",
        unit: "documentstore:document",
        scope: "pred collection = \"users\" and name = \"@me\"",
    },
    Grant {
        who: "all",
        verb: "put",
        unit: "documentstore:document",
        scope: "pred collection = \"tags\" and name = \"@me\"",
    },
    Grant {
        who: "all",
        verb: "see",
        unit: "Template",
        scope: "all",
    },
    Grant {
        who: "all",
        verb: "see",
        unit: "template:edition",
        scope: "all",
    },
    Grant {
        who: "all",
        verb: "see",
        unit: "documentstore:document",
        scope: "pred collection = \"schemas\"",
    },
    Grant {
        who: "all",
        verb: "put",
        unit: "Entry",
        scope: "pred owner = \"@me\"",
    },
];

pub fn stock(graph: &mut Graph) {
    keel_document::stock(graph);
    schema::stock(graph);
    entry::stock(graph);
    graph.add(
        Spec::build("Actor")
            .field("issuer", Kind::Text)
            .per("subject", Kind::Text, &["issuer"])
            .freeze()
            .seal(),
    );
}

pub(crate) fn graph() -> Graph {
    let mut graph = Graph::new();
    stock(&mut graph);
    graph
}

pub(crate) fn model() -> Result<Model, Error> {
    let users = Collection::new(
        "users",
        Shape::Object(vec![
            Field::required("owner", Shape::Text),
            Field::required("shared", Shape::Map(Box::new(Shape::Any))),
            Field::required(
                "apps",
                Shape::Map(Box::new(Shape::Map(Box::new(Shape::Any)))),
            ),
        ]),
    )?;
    let schemas = Collection::new(schema::COLLECTION, Shape::Any)?;
    let tags = Collection::new(entry::TAGS, Shape::Any)?;
    let entries = Collection::new(entry::ENTRIES, Shape::Any)?;
    Ok(Model::new(vec![users, schemas, tags, entries])?)
}

pub(crate) async fn seed(core: &keel::Core<keel::adapt::db::Sqlite>) -> Result<(), Error> {
    for grant in GRANTS {
        core.grant(keel::Grant::new(
            grant.who,
            grant.verb,
            grant.unit,
            grant.scope,
        ))
        .await?;
    }
    Ok(())
}
