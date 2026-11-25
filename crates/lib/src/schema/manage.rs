use super::{COLLECTION, Create, EDITION, Edition, Publish, seed};
use crate::{Error, Identity, Runtime, catalog};
use keel::{Op, form};

impl Runtime {
    pub async fn register(&self, identity: &Identity, input: &Create) -> Result<Edition, Error> {
        let actor = self.operator(identity).await?;
        if !catalog::slug(&input.name) {
            return Err(Error::Invalid("template name".into()));
        }
        let body = seed::envelope(&input.schema)?;
        let query = form("Template").when("name", Op::Eq, &input.name);
        if self.core.of(actor).one(&query).await?.is_some() {
            return Err(Error::Conflict);
        }
        let key = format!("{}/1", input.name);
        let document = self
            .store
            .of(actor)
            .create(COLLECTION, &key, body)
            .await
            .map_err(|error| Error::from(error).clash())?;
        let name = input.name.clone();
        let held = document.head.id.to_string();
        let bound = self
            .core
            .of(actor)
            .batch(async |tx| {
                let template = tx.put("Template", &[("name", name.as_str())]).await?;
                let id = template.to_string();
                tx.put(
                    EDITION,
                    &[
                        ("template", id.as_str()),
                        ("number", "1"),
                        ("document", held.as_str()),
                    ],
                )
                .await?;
                Ok(())
            })
            .await;
        if let Err(error) = bound {
            let _ = self.store.of(actor).end(&document.head).await;
            return Err(Error::from(error).clash());
        }
        Ok(Edition {
            template: input.name.clone(),
            number: 1,
            document: document.head.id,
            revision: document.head.token,
            version: 1,
            schema: input.schema.clone(),
        })
    }

    pub async fn release(
        &self,
        identity: &Identity,
        name: &str,
        input: &Publish,
    ) -> Result<Edition, Error> {
        let actor = self.operator(identity).await?;
        if name == "meta" {
            return Err(Error::Denied);
        }
        let template = self.lookup(actor, name).await?;
        let body = seed::envelope(&input.schema)?;
        let current = self.latest(actor, template.key()).await?;
        let latest = current.int("number").ok_or(Error::Corrupt)?;
        if input.base != latest {
            return Err(Error::Conflict);
        }
        let next = latest + 1;
        let key = format!("{name}/{next}");
        let document = self
            .store
            .of(actor)
            .create(COLLECTION, &key, body)
            .await
            .map_err(|error| Error::from(error).clash())?;
        let tid = template.key().to_string();
        let num = next.to_string();
        let held = document.head.id.to_string();
        let created = self
            .core
            .of(actor)
            .put(
                EDITION,
                &[
                    ("template", tid.as_str()),
                    ("number", num.as_str()),
                    ("document", held.as_str()),
                ],
            )
            .await;
        if let Err(error) = created {
            let _ = self.store.of(actor).end(&document.head).await;
            return Err(Error::from(error).clash());
        }
        Ok(Edition {
            template: name.into(),
            number: next,
            document: document.head.id,
            revision: document.head.token,
            version: 1,
            schema: input.schema.clone(),
        })
    }
}
