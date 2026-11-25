#![forbid(unsafe_code)]

mod auth;
mod catalog;
mod change;
mod entry;
mod error;
mod model;
mod runtime;
mod schema;
mod view;

pub use auth::{Auth, Identity};
pub use catalog::{Catalog, Context, Decision, Definition, Scopes, Slot, Summary};
pub use change::{Change, Patch, Scope, Write};
pub use entry::{Amend, Draft, Entry, Listed, Listing, Migrate, Tags};
pub use error::Error;
pub use model::stock;
pub use runtime::{Board, Runtime, Workbench};
pub use schema::{Brief, Create, Detail, Edition, Fault, Publish, Status, Template, Templates};
pub use view::{
    Basis, Declared, Device, Item, Presentation, Snapshot, Source, State, Tone, View, Views,
};

pub(crate) fn clock() -> Result<u64, Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|held| held.as_secs())
        .map_err(|_| Error::Unavailable)
}
