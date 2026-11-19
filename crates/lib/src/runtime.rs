use crate::{Catalog, Device, Error, Identity, State, Write, format, model, schema};
use keel::adapt::db::Sqlite;
use keel::{Core, Op, bind, bootstrap, form};
use keel_document::{Condition, Projection, Query, Record, Store};
use radicula_document::{Document, Operation, Path};
use serde_json::json;
use std::path::Path as Directory;
use std::sync::Arc;

pub struct Runtime {
    pub(crate) catalog: Catalog,
    core: Arc<Core<Sqlite>>,
    store: Store<Sqlite>,
    issuer: String,
}

impl Runtime {
    pub async fn health(&self) -> Result<(), Error> {
        self.core.anon().ask(&form("Actor").top(1)).await?;
        Ok(())
    }

    pub async fn create(root: &Directory, catalog: Catalog, issuer: String) -> Result<Self, Error> {
        let mut directory = std::fs::DirBuilder::new();
        directory.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            directory.mode(0o700);
        }
        directory.create(root)?;
        if std::fs::read_dir(root)?.next().is_some() {
            return Err(Error::Invalid("vacant data root required".into()));
        }
        let wire = Sqlite::file(&root.join("estate.sqlite3")).await?;
        let mut boot = bootstrap(model::graph(), wire)?;
        let token = boot.mint().await?;
        let core = Arc::new(boot.seal(&token).await?.identify("Actor")?);
        model::seed(&core).await?;
        let store = Store::create(core.clone(), model::model()?, &root.join("documents")).await?;
        schema::seed(&core, &store).await?;
        format::write(root)?;
        Ok(Self {
            core,
            store,
            catalog,
            issuer,
        })
    }

    pub async fn open(root: &Directory, catalog: Catalog, issuer: String) -> Result<Self, Error> {
        format::read(root)?;
        if !root.join("estate.sqlite3").is_file() {
            return Err(Error::Unavailable);
        }
        let wire = Sqlite::file(&root.join("estate.sqlite3")).await?;
        let core = Arc::new(bind(model::graph(), wire).await?.identify("Actor")?);
        let store = Store::open(core.clone(), model::model()?, &root.join("documents")).await?;
        schema::verify(&core, &store).await?;
        Ok(Self {
            core,
            store,
            catalog,
            issuer,
        })
    }

    pub async fn operator(&self, identity: &Identity) -> Result<i64, Error> {
        identity.current(&self.issuer)?;
        if let Some(id) = self.find(identity).await? {
            return Ok(id);
        }
        let result = self
            .core
            .anon()
            .put(
                "Actor",
                &[
                    ("issuer", identity.issuer()),
                    ("subject", identity.subject()),
                ],
            )
            .await;
        match result {
            Ok(id) => Ok(id),
            Err(error) => self.find(identity).await?.ok_or(Error::Engine(error)),
        }
    }

    async fn find(&self, identity: &Identity) -> Result<Option<i64>, Error> {
        let tree = form("Actor")
            .when("issuer", Op::Eq, identity.issuer())
            .when("subject", Op::Eq, identity.subject());
        Ok(self.core.anon().one(&tree).await?.map(|row| row.key()))
    }

    async fn record(&self, actor: i64) -> Result<Record, Error> {
        let query = Query {
            conditions: vec![Condition::Equal {
                path: Path::parse("/owner")?,
                value: actor.to_string().into(),
            }],
            select: vec![Projection {
                name: "body".into(),
                path: Path::root(),
            }],
        };
        let mut rows = self.store.of(actor).query("users", &query).await?;
        if rows.len() > 1 {
            return Err(Error::Corrupt);
        }
        if let Some(mut row) = rows.pop() {
            let value = Document::new(row.values.remove("body").ok_or(Error::Corrupt)?)?;
            return Ok(Record {
                head: row.head,
                value,
            });
        }
        let created = self
            .store
            .of(actor)
            .create(
                "users",
                &actor.to_string(),
                Document::new(json!({"owner": actor.to_string(), "shared": {}, "apps": {}}))?,
            )
            .await;
        match created {
            Ok(record) => Ok(record),
            Err(error) => {
                let mut rows = self.store.of(actor).query("users", &query).await?;
                if rows.len() != 1 {
                    return Err(error.into());
                }
                let mut row = rows.pop().ok_or(Error::Corrupt)?;
                Ok(Record {
                    head: row.head,
                    value: Document::new(row.values.remove("body").ok_or(Error::Corrupt)?)?,
                })
            }
        }
    }

    pub async fn read(&self, identity: &Identity, app: Option<&str>) -> Result<State, Error> {
        self.render(identity, app, Device::default()).await
    }

    pub async fn render(
        &self,
        identity: &Identity,
        app: Option<&str>,
        device: Device,
    ) -> Result<State, Error> {
        self.catalog.app(app)?;
        let actor = self.operator(identity).await?;
        let record = self.record(actor).await?;
        self.evaluate(identity, app, &record, device)
    }

    pub async fn write(
        &self,
        identity: &Identity,
        app: Option<&str>,
        write: &Write,
    ) -> Result<State, Error> {
        self.submit(identity, app, write, Device::default()).await
    }

    pub async fn submit(
        &self,
        identity: &Identity,
        app: Option<&str>,
        write: &Write,
        device: Device,
    ) -> Result<State, Error> {
        self.catalog.app(app)?;
        let actor = self.operator(identity).await?;
        let record = self.record(actor).await?;
        if record.head.token != write.revision {
            return Err(Error::Conflict);
        }
        if write.changes.is_empty() {
            return Err(Error::Invalid("empty change batch".into()));
        }
        let mut body = record.value.clone();
        for change in &write.changes {
            change.apply(&self.catalog, identity, &mut body)?;
        }
        let candidate = Record {
            head: record.head.clone(),
            value: body,
        };
        let mut state = self.evaluate(identity, app, &candidate, device)?;
        let operations = [Operation::Set {
            path: Path::root(),
            value: candidate.value.take(),
        }];
        let record = self.store.of(actor).edit(&record.head, &operations).await?;
        state.revision = record.head.token;
        Ok(state)
    }
}
