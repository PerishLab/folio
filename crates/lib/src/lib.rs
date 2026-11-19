#![forbid(unsafe_code)]

mod auth;
mod catalog;
mod change;
mod error;
mod format;
mod model;
mod runtime;
mod schema;
mod view;

pub use auth::{Auth, Identity};
pub use catalog::{Catalog, Context, Decision, Definition};
pub use change::{Change, Patch, Scope, Write};
pub use error::Error;
pub use model::stock;
pub use runtime::Runtime;
pub use view::{
    Basis, Declared, Device, Item, Presentation, Snapshot, Source, State, Tone, View, Views,
};

pub(crate) fn clock() -> Result<u64, Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|held| held.as_secs())
        .map_err(|_| Error::Unavailable)
}
