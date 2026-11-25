mod mutate;
mod store;
mod tags;
mod types;

pub use types::{Amend, Draft, Entry, Listed, Listing, Migrate, Tags};

use crate::{Error, Patch};
use keel::{Graph, atom::Kind, bond, spec::Spec};
use radicula_document::Operation;

pub(crate) const UNIT: &str = "Entry";
pub(crate) const TAGS: &str = "tags";
pub(crate) const ENTRIES: &str = "entries";

pub(crate) fn stock(graph: &mut Graph) {
    graph.add(
        Spec::build(UNIT)
            .field("owner", Kind::Text)
            .per("slug", Kind::Text, &["owner"])
            .bond("template", bond::Kind::Many2one, "Template", &[])
            .bond("edition", bond::Kind::Many2one, "template:edition", &[])
            .bond(
                "document",
                bond::Kind::Many2one,
                "documentstore:document",
                &[],
            )
            .seal(),
    );
}

pub(crate) fn reserved(id: &str) -> bool {
    matches!(id, "preferences" | "workbench" | "tags")
}

pub(crate) fn ops(patches: &[Patch]) -> Result<Vec<Operation>, Error> {
    if patches.is_empty() {
        return Err(Error::Invalid("empty operation batch".into()));
    }
    patches.iter().map(Patch::operation).collect()
}
