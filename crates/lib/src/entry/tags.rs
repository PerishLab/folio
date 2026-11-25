use super::{Amend, TAGS, Tags, ops};
use crate::{Error, Identity, Runtime, schema};
use keel::{Op, form};
use keel_document::Record;
use radicula_document::Document;
use serde_json::json;

const DOCUMENT: &str = "documentstore:document";

impl Runtime {
    pub(crate) async fn peek(&self, actor: i64) -> Result<Option<Record>, Error> {
        let query = form(DOCUMENT).when("collection", Op::Eq, TAGS).when(
            "name",
            Op::Eq,
            &actor.to_string(),
        );
        let Some(row) = self.core.of(actor).one(&query).await? else {
            return Ok(None);
        };
        Ok(Some(self.store.of(actor).read(TAGS, row.key()).await?))
    }

    async fn ensure(&self, actor: i64) -> Result<Record, Error> {
        if let Some(record) = self.peek(actor).await? {
            return Ok(record);
        }
        let initial = Document::new(json!({"tags": {}}))?;
        match self
            .store
            .of(actor)
            .create(TAGS, &actor.to_string(), initial)
            .await
        {
            Ok(record) => Ok(record),
            Err(error) => self.peek(actor).await?.ok_or_else(|| error.into()),
        }
    }

    pub async fn tags(&self, identity: &Identity) -> Result<Tags, Error> {
        let actor = self.operator(identity).await?;
        let record = self.ensure(actor).await?;
        Ok(Tags {
            revision: record.head.token,
            template: TAGS.into(),
            edition: 1,
            document: record.value.take(),
        })
    }

    pub async fn tag(&self, identity: &Identity, amend: &Amend) -> Result<Tags, Error> {
        let actor = self.operator(identity).await?;
        let operations = ops(&amend.operations)?;
        let record = self.ensure(actor).await?;
        if record.head.token != amend.revision {
            return Err(Error::Conflict);
        }
        let mut body = record.value;
        body.apply(&operations).map_err(Error::Patch)?;
        let template = self.lookup(actor, TAGS).await?;
        let edition = self.pick(actor, template.key(), 1).await?;
        let held = self.load(actor, &edition).await?;
        let validator = schema::compile(&held.schema)?;
        if !validator.is_valid(body.root()) {
            return Err(Error::Invalid("tags".into()));
        }
        let saved = self.store.of(actor).edit(&record.head, &operations).await?;
        Ok(Tags {
            revision: saved.head.token,
            template: TAGS.into(),
            edition: 1,
            document: saved.value.take(),
        })
    }
}
