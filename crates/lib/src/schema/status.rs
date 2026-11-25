use super::{Fault, Status, seed};
use crate::{Error, Identity, Runtime, entry, runtime};
use keel::{Op, form};
use std::collections::BTreeMap;

impl Runtime {
    pub async fn status(&self, identity: &Identity, name: &str) -> Result<Status, Error> {
        let actor = self.operator(identity).await?;
        let template = self.lookup(actor, name).await?;
        let editions = self.editions(actor, template.key()).await?;
        let latest = editions
            .last()
            .and_then(|row| row.int("number"))
            .ok_or(Error::Corrupt)?;
        let current = editions.last().ok_or(Error::Corrupt)?;
        let held = self.load(actor, current).await?;
        let validator = seed::compile(&held.schema)?;
        let mut numbers = BTreeMap::new();
        for row in &editions {
            let number = row.int("number").ok_or(Error::Corrupt)?;
            numbers.insert(row.key(), number);
        }
        let query = form(entry::UNIT).when("template", Op::Eq, &template.key().to_string());
        let pack = self.core.of(actor).ask(&query).await?;
        let mut rows = pack.rows().to_vec();
        rows.sort_by(|left, right| left.text("slug").cmp(&right.text("slug")));
        let mut checked = 0usize;
        let mut compatible = 0usize;
        let mut incompatible = Vec::new();
        for row in rows {
            let slug = row.text("slug").ok_or(Error::Corrupt)?;
            let doc = row.int("document").ok_or(Error::Corrupt)?;
            let edition = row
                .int("edition")
                .and_then(|key| numbers.get(&key).copied())
                .ok_or(Error::Corrupt)?;
            let record = self.store.of(actor).read(entry::ENTRIES, doc).await?;
            checked += 1;
            match validator.validate(record.value.root()) {
                Ok(()) => compatible += 1,
                Err(error) => incompatible.push(Fault {
                    id: slug.into(),
                    edition,
                    reason: error.to_string(),
                }),
            }
        }
        if name == entry::TAGS
            && let Some(record) = self.peek(actor).await?
        {
            checked += 1;
            match validator.validate(record.value.root()) {
                Ok(()) => compatible += 1,
                Err(error) => incompatible.push(Fault {
                    id: entry::TAGS.into(),
                    edition: 1,
                    reason: error.to_string(),
                }),
            }
        }
        Ok(Status {
            template: name.into(),
            latest,
            format: runtime::format::NAME.into(),
            ready: incompatible.is_empty(),
            checked,
            compatible,
            incompatible,
        })
    }
}
