use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dm_core::{config::Config, search::SearchQuery, Core};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "directive-memory", version, about = "AI-native personal knowledge base")]
struct Cli {
    /// Path to config file (TOML). Env vars prefixed DM_* override.
    #[arg(long, short)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start the HTTP server (REST API + web UI).
    Serve,
    /// Start the MCP server on stdio.
    Mcp,
    /// Force a full reindex and exit.
    Reindex,
    /// Run a one-shot search and print JSON.
    Search {
        query: String,
        #[arg(long, default_value_t = 5)]
        top_k: i64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Route logs to stderr. stdout is reserved for `mcp` JSON-RPC and `search`
    // JSON output; any stray log line there corrupts the protocol.
    use std::io::IsTerminal;
    let ansi = std::io::stderr().is_terminal();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(ansi)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into())
        )
        .init();

    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref()).context("load config")?;
    let core = Core::open(config).await.context("open core")?;

    match cli.command {
        Command::Serve => serve(core).await?,
        Command::Mcp => {
            // Startup reindex so files edited while the MCP client wasn't
            // connected are searchable on the first tool call. Mtime-diffed,
            // so this is near-instant on a stable corpus.
            let r = core.reindex().await?;
            tracing::info!(indexed = r.files_indexed, pruned = r.files_pruned, "mcp startup reindex");
            dm_mcp::run_stdio(core).await?
        }
        Command::Reindex => {
            let r = core.reindex().await?;
            println!("indexed {} files, pruned {}", r.files_indexed, r.files_pruned);
        }
        Command::Search { query, top_k } => {
            let hits = core.search(&SearchQuery { query, top_k, ..Default::default() }).await?;
            println!("{}", serde_json::to_string_pretty(&hits)?);
        }
    }
    Ok(())
}

async fn serve(core: Core) -> Result<()> {
    // Startup reindex so freshly-added markdown files are searchable.
    let report = core.reindex().await?;
    tracing::info!(indexed = report.files_indexed, pruned = report.files_pruned, "startup reindex");
    let addr: std::net::SocketAddr = format!("{}:{}", core.config.bind, core.config.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");
    let router = dm_api::build_router(core);
    axum::serve(listener, router).await?;
    Ok(())
}
