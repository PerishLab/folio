mod manage;
mod seed;
mod status;
mod types;

pub(crate) use seed::{COLLECTION, EDITION, compile, seed, stock, verify};
pub use types::{Brief, Create, Detail, Edition, Fault, Publish, Status, Template, Templates};

use crate::{Error, Identity, Runtime};
use keel::{Op, Row, form};
use radicula_document::Value;
use std::collections::BTreeMap;

pub(crate) struct Held {
    pub(crate) document: i64,
    pub(crate) revision: String,
    pub(crate) version: i64,
    pub(crate) schema: Value,
}

impl Runtime {
    pub async fn templates(&self, identity: &Identity) -> Result<Templates, Error> {
        let actor = self.operator(identity).await?;
        let templates = self.core.of(actor).live("Template").await?;
        let editions = self.core.of(actor).live(EDITION).await?;
        let mut grouped: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        for row in editions {
            let parent = row.int("template").ok_or(Error::Corrupt)?;
            let number = row.int("number").ok_or(Error::Corrupt)?;
            grouped.entry(parent).or_default().push(number);
        }
        let mut list = Vec::with_capacity(templates.len());
        for row in templates {
            let name = row.text("name").ok_or(Error::Corrupt)?.to_string();
            let mut numbers = grouped.remove(&row.key()).unwrap_or_default();
            numbers.sort_unstable();
            let latest = numbers.last().copied().ok_or(Error::Corrupt)?;
            list.push(Template {
                builtin: seed::builtin(&name),
                name,
                latest,
                editions: numbers,
            });
        }
        list.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(Templates { templates: list })
    }

    pub async fn template(&self, identity: &Identity, name: &str) -> Result<Detail, Error> {
        let actor = self.operator(identity).await?;
        let template = self.lookup(actor, name).await?;
        let rows = self.editions(actor, template.key()).await?;
        let mut editions = Vec::with_capacity(rows.len());
        for row in &rows {
            let number = row.int("number").ok_or(Error::Corrupt)?;
            let held = self.load(actor, row).await?;
            editions.push(Brief {
                number,
                document: held.document,
                revision: held.revision,
                version: held.version,
            });
        }
        let latest = editions
            .last()
            .map(|item| item.number)
            .ok_or(Error::Corrupt)?;
        Ok(Detail {
            name: name.into(),
            builtin: seed::builtin(name),
            latest,
            editions,
        })
    }

    pub async fn edition(
        &self,
        identity: &Identity,
        name: &str,
        number: i64,
    ) -> Result<Edition, Error> {
        let actor = self.operator(identity).await?;
        let template = self.lookup(actor, name).await?;
        let row = self.pick(actor, template.key(), number).await?;
        let held = self.load(actor, &row).await?;
        Ok(Edition {
            template: name.into(),
            number,
            document: held.document,
            revision: held.revision,
            version: held.version,
            schema: held.schema,
        })
    }

    pub(crate) async fn lookup(&self, actor: i64, name: &str) -> Result<Row, Error> {
        let query = form("Template").when("name", Op::Eq, name);
        self.core.of(actor).one(&query).await?.ok_or(Error::Missing)
    }

    pub(crate) async fn editions(&self, actor: i64, template: i64) -> Result<Vec<Row>, Error> {
        let query = form(EDITION).when("template", Op::Eq, &template.to_string());
        let pack = self.core.of(actor).ask(&query).await?;
        let mut rows = pack.rows().to_vec();
        rows.sort_by_key(|row| row.int("number").unwrap_or(0));
        if rows.is_empty() {
            return Err(Error::Corrupt);
        }
        Ok(rows)
    }

    pub(crate) async fn pick(&self, actor: i64, template: i64, number: i64) -> Result<Row, Error> {
        let query = form(EDITION)
            .when("template", Op::Eq, &template.to_string())
            .when("number", Op::Eq, &number.to_string());
        self.core.of(actor).one(&query).await?.ok_or(Error::Missing)
    }

    pub(crate) async fn latest(&self, actor: i64, template: i64) -> Result<Row, Error> {
        self.editions(actor, template)
            .await?
            .pop()
            .ok_or(Error::Corrupt)
    }

    pub(crate) async fn load(&self, actor: i64, row: &Row) -> Result<Held, Error> {
        let document = row.int("document").ok_or(Error::Corrupt)?;
        let record = self.store.of(actor).read(COLLECTION, document).await?;
        let (version, schema) = seed::unwrap(record.value.root())?;
        Ok(Held {
            document,
            revision: record.head.token,
            version,
            schema,
        })
    }
}
