#![forbid(unsafe_code)]

mod config;

use clap::{Parser, Subcommand};
use folio::{Catalog, Runtime};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "folio-api", version = plumb::version!("FOLIO"))]
struct Cli {
    #[command(subcommand)]
    command: Command,
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
    match cli.command {
        Command::Bootstrap { root } => bootstrap(root).await,
        Command::Serve { root } => serve(root).await,
    }
}

async fn bootstrap(root: PathBuf) -> Result<(), String> {
    let settings = config::Settings::load(&root)?;
    let data = settings.data(&root);
    if data.join("estate.sqlite3").is_file() {
        Runtime::open(&data, Catalog::themes(), settings.issuer)
            .await
            .map_err(|error| error.to_string())?;
    } else {
        Runtime::create(&data, Catalog::themes(), settings.issuer)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

async fn serve(root: PathBuf) -> Result<(), String> {
    let settings = config::Settings::load(&root)?;
    let auth = settings.auth(&root)?;
    let runtime = Runtime::open(&settings.data(&root), Catalog::themes(), settings.issuer)
        .await
        .map_err(|error| error.to_string())?;
    let listener = tokio::net::TcpListener::bind(&settings.listen)
        .await
        .map_err(|error| error.to_string())?;
    eprintln!(
        "folio-api: listening {}",
        listener.local_addr().map_err(|error| error.to_string())?
    );
    axum::serve(listener, api::router(runtime, auth))
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
