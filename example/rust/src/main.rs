use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use example_api::api::messages::CollectResponse;
use futures_util::StreamExt;
use std::path::PathBuf;
use tracing::debug;

#[derive(Debug, Parser)]
#[command(name = "rustex-example-cli")]
#[command(about = "Typed Rust CLI for the Convex example using Rustex-generated bindings")]
struct Cli {
    #[arg(long, env = "CONVEX_URL")]
    deployment_url: Option<String>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Add {
        #[arg(long)]
        author: String,
        #[arg(long)]
        body: String,
    },
    List,
    Watch {
        #[arg(long, default_value_t = 3)]
        updates: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    load_example_env();
    let _ = example_api::init_default_tracing();
    let cli = Cli::parse();
    let deployment_url = cli
        .deployment_url
        .or_else(|| std::env::var("CONVEX_URL").ok())
        .context("missing Convex deployment URL; pass --deployment-url or set CONVEX_URL")?;

    match cli.command {
        Command::Add { author, body } => add_message(&deployment_url, author, body).await,
        Command::List => list_messages(&deployment_url).await,
        Command::Watch { updates } => watch_messages(&deployment_url, updates).await,
    }
}

fn load_example_env() {
    let env_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(".env.local");
    let _ = dotenvy::from_path(&env_path);
    debug!(env_path = %env_path.display(), "loaded example environment file if present");
}

async fn add_message(deployment_url: &str, author: String, body: String) -> Result<()> {
    let mut client = example_api::RustexClient::new(deployment_url)
        .await
        .context("failed to connect typed Convex client")?;
    let id = example_api::mutation!(client, messages::add, { author, body })
        .await
        .context("typed add mutation failed")?;

    println!("inserted message id: {}", id.0);
    Ok(())
}

async fn list_messages(deployment_url: &str) -> Result<()> {
    let mut client = example_api::RustexClient::new(deployment_url)
        .await
        .context("failed to connect typed Convex client")?;
    let messages = example_api::query!(client, messages::collect, {})
        .await
        .context("typed collect query failed")?;

    print_json_messages("messages", &messages);
    Ok(())
}

async fn watch_messages(deployment_url: &str, updates: usize) -> Result<()> {
    let mut client = example_api::RustexClient::new(deployment_url)
        .await
        .context("failed to connect typed Convex client")?;
    let mut subscription = example_api::subscribe!(client, messages::collect, {})
        .await
        .context("failed to subscribe to typed messages:collect")?;

    for index in 1..=updates {
        let snapshot = subscription
            .next()
            .await
            .context("subscription ended before delivering the requested updates")?
            .context("typed collect subscription returned an error")?;
        let messages: CollectResponse = snapshot;
        println!("update #{index}");
        print_json_messages("messages", &messages);
    }

    Ok(())
}

fn print_json_messages(label: &str, messages: &CollectResponse) {
    println!("{label}: {}", messages.len());
    for message in messages {
        println!(
            "- {} [{}] {}: {}",
            message.id.0, message.creation_time, message.author, message.body
        );
    }
}
