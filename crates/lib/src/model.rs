use crate::Error;
use keel::{Graph, atom::Kind, spec::Spec};
use keel_document::{Collection, Field, Model, Shape};

pub fn stock(graph: &mut Graph) {
    keel_document::stock(graph);
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
    Ok(Model::new(vec![Collection::new(
        "users",
        Shape::Object(vec![
            Field::required("owner", Shape::Text),
            Field::required("shared", Shape::Map(Box::new(Shape::Any))),
            Field::required(
                "apps",
                Shape::Map(Box::new(Shape::Map(Box::new(Shape::Any)))),
            ),
        ]),
    )?])?)
}

pub(crate) async fn seed(core: &keel::Core<keel::adapt::db::Sqlite>) -> Result<(), Error> {
    for verb in ["put", "see"] {
        core.put(
            "@grant",
            &[
                ("who", "anon"),
                ("verb", verb),
                ("unit", "Actor"),
                ("scope", "all"),
            ],
        )
        .await?;
    }
    core.put(
        "@grant",
        &[
            ("who", "all"),
            ("verb", "put"),
            ("unit", "documentstore:document"),
            ("scope", "pred name = \"@me\""),
        ],
    )
    .await?;
    Ok(())
}
