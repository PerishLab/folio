use super::{Amend, ENTRIES, Entry, Migrate, UNIT, ops};
use crate::{Error, Identity, Runtime, schema};
use keel::{Op, form};

const DOCUMENT: &str = "documentstore:document";

struct Move {
    entry: i64,
    template: i64,
    from: i64,
    to: i64,
    document: i64,
    head: String,
}

impl Runtime {
    pub async fn amend(
        &self,
        identity: &Identity,
        id: &str,
        amend: &Amend,
    ) -> Result<Entry, Error> {
        let actor = self.operator(identity).await?;
        let operations = ops(&amend.operations)?;
        let row = self.owned(actor, id).await?;
        let (template, edition, doc) = self.describe(actor, &row).await?;
        let record = self.store.of(actor).read(ENTRIES, doc).await?;
        if record.head.token != amend.revision {
            return Err(Error::Conflict);
        }
        let mut body = record.value;
        body.apply(&operations).map_err(Error::Patch)?;
        let eid = row.int("edition").ok_or(Error::Corrupt)?;
        let bound = self
            .core
            .of(actor)
            .one(&form(schema::EDITION).when("id", Op::Eq, &eid.to_string()))
            .await?
            .ok_or(Error::Corrupt)?;
        let held = self.load(actor, &bound).await?;
        let validator = schema::compile(&held.schema)?;
        if !validator.is_valid(body.root()) {
            return Err(Error::Invalid("document".into()));
        }
        let saved = self.store.of(actor).edit(&record.head, &operations).await?;
        Ok(Entry {
            revision: saved.head.token,
            id: id.into(),
            template,
            edition,
            document: saved.value.take(),
        })
    }

    pub async fn migrate(
        &self,
        identity: &Identity,
        id: &str,
        input: &Migrate,
    ) -> Result<Entry, Error> {
        let actor = self.operator(identity).await?;
        let row = self.owned(actor, id).await?;
        let (template, _, doc) = self.describe(actor, &row).await?;
        let record = self.store.of(actor).read(ENTRIES, doc).await?;
        if record.head.token != input.revision {
            return Err(Error::Conflict);
        }
        let tid = row.int("template").ok_or(Error::Corrupt)?;
        let from = row.int("edition").ok_or(Error::Corrupt)?;
        let latest = self.latest(actor, tid).await?;
        let number = latest.int("number").ok_or(Error::Corrupt)?;
        let held = self.load(actor, &latest).await?;
        let validator = schema::compile(&held.schema)?;
        if !validator.is_valid(record.value.root()) {
            return Err(Error::Invalid("incompatible".into()));
        }
        let bumped = self.store.of(actor).edit(&record.head, &[]).await?;
        let plan = Move {
            entry: row.key(),
            template: tid,
            from,
            to: latest.key(),
            document: doc,
            head: bumped.head.token.clone(),
        };
        self.advance(actor, &plan).await?;
        Ok(Entry {
            revision: bumped.head.token,
            id: id.into(),
            template,
            edition: number,
            document: bumped.value.take(),
        })
    }

    async fn advance(&self, actor: i64, plan: &Move) -> Result<(), Error> {
        let key = plan.entry.to_string();
        let tid = plan.template.to_string();
        let to = plan.to.to_string();
        let did = plan.document.to_string();
        let mut fault: Option<Error> = None;
        let result = self
            .core
            .of(actor)
            .batch(async |tx| {
                let Some(entry) = tx.one(&form(UNIT).when("id", Op::Eq, key.as_str())).await?
                else {
                    fault = Some(Error::Missing);
                    return Err(keel::adapt::Error::Adapt("missing".into()));
                };
                if entry.int("edition") != Some(plan.from) {
                    fault = Some(Error::Conflict);
                    return Err(keel::adapt::Error::Adapt("conflict".into()));
                }
                let query = form(DOCUMENT).when("id", Op::Eq, did.as_str()).when(
                    "collection",
                    Op::Eq,
                    ENTRIES,
                );
                let Some(current) = tx.one(&query).await? else {
                    fault = Some(Error::Missing);
                    return Err(keel::adapt::Error::Adapt("missing".into()));
                };
                if current.text("head") != Some(plan.head.as_str()) {
                    fault = Some(Error::Conflict);
                    return Err(keel::adapt::Error::Adapt("conflict".into()));
                }
                let editions = tx
                    .ask(&form(schema::EDITION).when("template", Op::Eq, tid.as_str()))
                    .await?;
                let top = editions
                    .rows()
                    .iter()
                    .max_by_key(|item| item.int("number").unwrap_or(0))
                    .map(keel::Row::key);
                if top != Some(plan.to) {
                    fault = Some(Error::Conflict);
                    return Err(keel::adapt::Error::Adapt("conflict".into()));
                }
                if plan.from != plan.to {
                    tx.set(UNIT, plan.entry, &[("edition", to.as_str())])
                        .await?;
                }
                Ok(())
            })
            .await;
        match result {
            Ok(()) => Ok(()),
            Err(error) => Err(fault.unwrap_or_else(|| error.into())),
        }
    }

    pub async fn remove(&self, identity: &Identity, id: &str, revision: &str) -> Result<(), Error> {
        let actor = self.operator(identity).await?;
        let owner = actor.to_string();
        let slug = id.to_string();
        let expected = revision.to_string();
        let mut fault: Option<Error> = None;
        let result = self
            .core
            .of(actor)
            .batch(async |tx| {
                let query = form(UNIT).when("owner", Op::Eq, owner.as_str()).when(
                    "slug",
                    Op::Eq,
                    slug.as_str(),
                );
                let Some(entry) = tx.one(&query).await? else {
                    fault = Some(Error::Missing);
                    return Err(keel::adapt::Error::Adapt("missing".into()));
                };
                let Some(doc) = entry.int("document") else {
                    fault = Some(Error::Corrupt);
                    return Err(keel::adapt::Error::Adapt("corrupt".into()));
                };
                let did = doc.to_string();
                let check = form(DOCUMENT).when("id", Op::Eq, did.as_str()).when(
                    "collection",
                    Op::Eq,
                    ENTRIES,
                );
                let Some(held) = tx.one(&check).await? else {
                    fault = Some(Error::Missing);
                    return Err(keel::adapt::Error::Adapt("missing".into()));
                };
                if held.text("head") != Some(expected.as_str()) {
                    fault = Some(Error::Conflict);
                    return Err(keel::adapt::Error::Adapt("conflict".into()));
                }
                tx.end(UNIT, entry.key()).await?;
                tx.end(DOCUMENT, doc).await?;
                Ok((entry.key(), doc))
            })
            .await;
        let (entry, doc) = match result {
            Ok(pair) => pair,
            Err(error) => return Err(fault.unwrap_or_else(|| error.into())),
        };
        self.retire(UNIT, entry).await?;
        self.retire(DOCUMENT, doc).await?;
        Ok(())
    }

    async fn retire(&self, unit: &str, key: i64) -> Result<(), Error> {
        let scope = format!("row {key}");
        let lower = unit.to_ascii_lowercase();
        let query = form("@grant").when("unit", Op::Eq, lower.as_str()).when(
            "scope",
            Op::Eq,
            scope.as_str(),
        );
        let rows = self.core.ask(&query).await?;
        for row in rows.rows() {
            self.core.end("@grant", row.key()).await?;
        }
        Ok(())
    }
}
