#![forbid(unsafe_code)]

use api::config;
use clap::{Parser, Subcommand};
use folio::Runtime;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "folio-api", version = plumb::version!("FOLIO"))]
struct Cli {
    #[command(subcommand)]
    command: Command,
    #[arg(long = "sidecar-stamp", hide = true, global = true)]
    stamp: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    Bootstrap { root: PathBuf },
    Serve { root: PathBuf },
}

#[tokio::main]
async fn main() {
    let result = run().await;
    if let Err(error) = result {
        eprintln!("folio-api: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    plumb::identity!("FOLIO")?;
    let cli = Cli::parse();
    plumb::identity::ready()?;
    let _stamp = cli.stamp;
    match cli.command {
        Command::Bootstrap { root } => bootstrap(root).await,
        Command::Serve { root } => serve(root).await,
    }
}

async fn bootstrap(root: PathBuf) -> Result<(), String> {
    let settings = config::Settings::load(&root)?;
    let catalog = settings.catalog()?;
    let data = settings.data(&root);
    if data.join("estate.sqlite3").is_file() {
        Runtime::open(&data, catalog, settings.issuer)
            .await
            .map_err(|error| error.to_string())?;
    } else {
        Runtime::create(&data, catalog, settings.issuer)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn serve(root: PathBuf) -> Result<(), String> {
    let settings = config::Settings::load(&root)?;
    let catalog = settings.catalog()?;
    let auth = settings.auth(&root)?;
    let relay = settings.relay(&root)?;
    let runtime = Runtime::open(&settings.data(&root), catalog, settings.issuer)
        .await
        .map_err(|error| error.to_string())?;
    let listener = tokio::net::TcpListener::bind(&settings.listen)
        .await
        .map_err(|error| error.to_string())?;
    let live = listener.local_addr().map_err(|error| error.to_string())?;
    eprintln!(
        "{}",
        serde_json::json!({ "role": "api", "endpoint": format!("http://{live}") })
    );
    let app = api::mount(runtime, auth, relay);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown())
    .await
    .map_err(|error| error.to_string())
}

#[cfg(unix)]
async fn shutdown() {
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("SIGTERM handler");
    tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = term.recv() => {} }
}

#[cfg(not(unix))]
async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
