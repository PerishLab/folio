use super::{Draft, ENTRIES, Entry, Listed, Listing, UNIT, reserved};
use crate::{Error, Identity, Runtime, catalog, schema};
use keel::{Op, Row, form};
use radicula_document::Document;
use std::collections::BTreeMap;

const DOCUMENT: &str = "documentstore:document";

struct Bound {
    template: i64,
    edition: i64,
    document: i64,
}

impl Runtime {
    pub async fn entries(&self, identity: &Identity) -> Result<Listing, Error> {
        let actor = self.operator(identity).await?;
        let query = form(UNIT).when("owner", Op::Eq, &actor.to_string());
        let rows = self.core.of(actor).ask(&query).await?;
        let mut names = BTreeMap::new();
        for row in self.core.of(actor).live("Template").await? {
            let name = row.text("name").ok_or(Error::Corrupt)?.to_string();
            names.insert(row.key(), name);
        }
        let mut numbers = BTreeMap::new();
        for row in self.core.of(actor).live(schema::EDITION).await? {
            let number = row.int("number").ok_or(Error::Corrupt)?;
            numbers.insert(row.key(), number);
        }
        let mut documents = Vec::with_capacity(rows.rows().len());
        for row in rows.rows() {
            let id = row.text("slug").ok_or(Error::Corrupt)?.to_string();
            let template = row
                .int("template")
                .and_then(|key| names.get(&key).cloned())
                .ok_or(Error::Corrupt)?;
            let edition = row
                .int("edition")
                .and_then(|key| numbers.get(&key).copied())
                .ok_or(Error::Corrupt)?;
            let doc = row.int("document").ok_or(Error::Corrupt)?;
            let head = self.head(actor, doc).await?;
            documents.push(Listed {
                id,
                template,
                edition,
                revision: head,
            });
        }
        documents.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Listing { documents })
    }

    pub async fn entry(&self, identity: &Identity, id: &str) -> Result<Entry, Error> {
        let actor = self.operator(identity).await?;
        let row = self.owned(actor, id).await?;
        let (template, edition, doc) = self.describe(actor, &row).await?;
        let record = self.store.of(actor).read(ENTRIES, doc).await?;
        Ok(Entry {
            revision: record.head.token,
            id: id.into(),
            template,
            edition,
            document: record.value.take(),
        })
    }

    pub async fn insert(&self, identity: &Identity, draft: &Draft) -> Result<Entry, Error> {
        let actor = self.operator(identity).await?;
        if !catalog::slug(&draft.id) || reserved(&draft.id) {
            return Err(Error::Invalid("document id".into()));
        }
        if draft.template == "meta" {
            return Err(Error::Invalid("meta template".into()));
        }
        let template = self.lookup(actor, &draft.template).await?;
        let edition = match draft.edition {
            Some(number) => self.pick(actor, template.key(), number).await?,
            None => self.latest(actor, template.key()).await?,
        };
        let number = edition.int("number").ok_or(Error::Corrupt)?;
        let held = self.load(actor, &edition).await?;
        let validator = schema::compile(&held.schema)?;
        if !validator.is_valid(&draft.document) {
            return Err(Error::Invalid("document".into()));
        }
        let body = Document::new(draft.document.clone())?;
        match self.owned(actor, &draft.id).await {
            Ok(_) => return Err(Error::Conflict),
            Err(Error::Missing) => {}
            Err(error) => return Err(error),
        }
        let probe = Bound {
            template: template.key(),
            edition: edition.key(),
            document: held.document,
        };
        self.permit(actor, &draft.id, &probe).await?;
        let key = format!("{actor}/{}", draft.id);
        let record = self
            .store
            .sudo()
            .create(ENTRIES, &key, body)
            .await
            .map_err(|error| Error::from(error).clash())?;
        let who = actor.to_string();
        let scope = format!("row {}", record.head.id);
        let grant = self
            .core
            .put(
                "@grant",
                &[
                    ("who", who.as_str()),
                    ("verb", "*"),
                    ("unit", DOCUMENT),
                    ("scope", scope.as_str()),
                ],
            )
            .await;
        let grant = match grant {
            Ok(id) => id,
            Err(error) => {
                self.store.sudo().end(&record.head).await?;
                return Err(error.into());
            }
        };
        let tid = template.key().to_string();
        let eid = edition.key().to_string();
        let did = record.head.id.to_string();
        let created = self
            .core
            .of(actor)
            .put(
                UNIT,
                &[
                    ("owner", who.as_str()),
                    ("slug", draft.id.as_str()),
                    ("template", tid.as_str()),
                    ("edition", eid.as_str()),
                    ("document", did.as_str()),
                ],
            )
            .await;
        if let Err(error) = created {
            self.core.end("@grant", grant).await?;
            self.store.sudo().end(&record.head).await?;
            return Err(Error::from(error).clash());
        }
        Ok(Entry {
            revision: record.head.token,
            id: draft.id.clone(),
            template: draft.template.clone(),
            edition: number,
            document: record.value.take(),
        })
    }

    async fn permit(&self, actor: i64, slug: &str, bound: &Bound) -> Result<(), Error> {
        let who = actor.to_string();
        let tid = bound.template.to_string();
        let eid = bound.edition.to_string();
        let did = bound.document.to_string();
        let mut allowed = false;
        let probe = self
            .core
            .of(actor)
            .batch(async |tx| {
                tx.put(
                    UNIT,
                    &[
                        ("owner", who.as_str()),
                        ("slug", slug),
                        ("template", tid.as_str()),
                        ("edition", eid.as_str()),
                        ("document", did.as_str()),
                    ],
                )
                .await?;
                allowed = true;
                Err::<(), _>(keel::adapt::Error::Adapt("probe".into()))
            })
            .await;
        if allowed {
            return Ok(());
        }
        match probe {
            Ok(()) => Ok(()),
            Err(error) => Err(Error::from(error).clash()),
        }
    }

    pub(crate) async fn owned(&self, actor: i64, id: &str) -> Result<Row, Error> {
        let query = form(UNIT)
            .when("owner", Op::Eq, &actor.to_string())
            .when("slug", Op::Eq, id);
        self.core.of(actor).one(&query).await?.ok_or(Error::Missing)
    }

    pub(crate) async fn describe(
        &self,
        actor: i64,
        row: &Row,
    ) -> Result<(String, i64, i64), Error> {
        let tid = row.int("template").ok_or(Error::Corrupt)?;
        let eid = row.int("edition").ok_or(Error::Corrupt)?;
        let did = row.int("document").ok_or(Error::Corrupt)?;
        let template = self
            .core
            .of(actor)
            .one(&form("Template").when("id", Op::Eq, &tid.to_string()))
            .await?
            .and_then(|item| item.text("name").map(str::to_string))
            .ok_or(Error::Corrupt)?;
        let edition = self
            .core
            .of(actor)
            .one(&form(schema::EDITION).when("id", Op::Eq, &eid.to_string()))
            .await?
            .and_then(|item| item.int("number"))
            .ok_or(Error::Corrupt)?;
        Ok((template, edition, did))
    }

    async fn head(&self, actor: i64, doc: i64) -> Result<String, Error> {
        let query =
            form(DOCUMENT)
                .when("id", Op::Eq, &doc.to_string())
                .when("collection", Op::Eq, ENTRIES);
        let row = self
            .core
            .of(actor)
            .one(&query)
            .await?
            .ok_or(Error::Missing)?;
        row.text("head").map(str::to_string).ok_or(Error::Corrupt)
    }
}
