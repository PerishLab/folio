use crate::{Device, Error, Identity, Runtime, Scopes, View, Write};
use radicula_document::Value;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Board {
    pub scopes: Vec<String>,
    pub shared: View,
    pub apps: BTreeMap<String, View>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Workbench {
    pub revision: String,
    pub document: Board,
}

impl Runtime {
    pub fn scopes(&self) -> Scopes {
        self.catalog.scopes()
    }

    pub async fn workbench(&self, identity: &Identity, device: Device) -> Result<Workbench, Error> {
        let actor = self.operator(identity).await?;
        let record = self.record(actor).await?;
        let document = self.board(identity, record.value.root(), device)?;
        Ok(Workbench {
            revision: record.head.token,
            document,
        })
    }

    pub async fn revise(
        &self,
        identity: &Identity,
        write: &Write,
        device: Device,
    ) -> Result<Workbench, Error> {
        let (document, revision) = self
            .publish(identity, write, |body| self.board(identity, body, device))
            .await?;
        Ok(Workbench { revision, document })
    }

    fn board(&self, identity: &Identity, body: &Value, device: Device) -> Result<Board, Error> {
        let shared = self.evaluate(identity, None, body, device)?;
        let mut apps = BTreeMap::new();
        for name in self.catalog.apps() {
            apps.insert(
                name.clone(),
                self.evaluate(identity, Some(name), body, device)?,
            );
        }
        Ok(Board {
            scopes: self.catalog.apps().to_vec(),
            shared,
            apps,
        })
    }
}
