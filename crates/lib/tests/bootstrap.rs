use folio::{Catalog, Error, Runtime};
use keel::{Core, Graph, adapt::db::Sqlite, bind};
use std::path::Path;

const ISSUER: &str = "https://identity.test/api";

async fn create(root: &Path) -> Runtime {
    Runtime::create(root, Catalog::themes(), ISSUER.into())
        .await
        .unwrap()
}

async fn open(root: &Path) -> Result<Runtime, Error> {
    Runtime::open(root, Catalog::themes(), ISSUER.into()).await
}

async fn core(root: &Path) -> Core<Sqlite> {
    let mut graph = Graph::new();
    folio::stock(&mut graph);
    let wire = Sqlite::file(&root.join("estate.sqlite3")).await.unwrap();
    bind(graph, wire).await.unwrap()
}

async fn names(core: &Core<Sqlite>) -> Vec<(i64, i64)> {
    let mut rows: Vec<_> = core
        .live("template:edition")
        .await
        .unwrap()
        .into_iter()
        .map(|row| {
            assert_eq!(row.int("number"), Some(1));
            (row.key(), row.int("document").unwrap())
        })
        .collect();
    rows.sort_unstable();
    rows
}

#[tokio::test]
async fn ready() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path();
    let runtime = create(root).await;
    runtime.health().await.unwrap();
    drop(runtime);
    let core = core(root).await;
    let before = names(&core).await;
    assert_eq!(before.len(), 2);
    assert_ne!(before[0].1, before[1].1);
    drop(core);
    let runtime = open(root).await.unwrap();
    runtime.health().await.unwrap();
    drop(runtime);
    let core = self::core(root).await;
    assert_eq!(names(&core).await, before);
}

#[tokio::test]
async fn partial() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path();
    let estate = root.join("estate.sqlite3");
    std::fs::write(&estate, b"preceding format").unwrap();
    assert!(matches!(open(root).await, Err(Error::Invalid(_))));
    assert_eq!(std::fs::read(&estate).unwrap(), b"preceding format");
}

#[tokio::test]
async fn missing() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path();
    drop(create(root).await);
    let core = core(root).await;
    let row = names(&core).await[0].0;
    core.end("template:edition", row).await.unwrap();
    drop(core);
    assert!(matches!(open(root).await, Err(Error::Corrupt)));
}
