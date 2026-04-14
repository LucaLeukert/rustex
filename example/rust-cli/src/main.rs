use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use convex::ConvexClient;
use example_api::api::messages::{
    self, AddArgs, Collect, CollectResponse, FindByAuthorArgs, MultiReturnDemoResponse,
};
use example_api::ids::MessagesId;
use futures_util::StreamExt;
use rustex_runtime::{FunctionSpec, TypedConvexClient, decode_result, encode_args};
use std::path::PathBuf;

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
    Find {
        #[arg(long)]
        author: String,
    },
    Status,
    Watch {
        #[arg(long, default_value_t = 3)]
        updates: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    load_example_env();
    let cli = Cli::parse();
    let deployment_url = cli
        .deployment_url
        .or_else(|| std::env::var("CONVEX_URL").ok())
        .context("missing Convex deployment URL; pass --deployment-url or set CONVEX_URL")?;

    match cli.command {
        Command::Add { author, body } => add_message(&deployment_url, author, body).await,
        Command::List => list_messages(&deployment_url).await,
        Command::Find { author } => find_messages(&deployment_url, author).await,
        Command::Status => show_status(&deployment_url).await,
        Command::Watch { updates } => watch_messages(&deployment_url, updates).await,
    }
}

fn load_example_env() {
    let env_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.env.local");
    let _ = dotenvy::from_path(env_path);
}

async fn add_message(deployment_url: &str, author: String, body: String) -> Result<()> {
    let mut client = TypedConvexClient::new(deployment_url)
        .await
        .context("failed to connect typed Convex client")?;
    let id = client
        .mutation(messages::add(), &AddArgs { author, body })
        .await
        .context("typed add mutation failed")?;

    print_inserted_id(&id);
    Ok(())
}

async fn list_messages(deployment_url: &str) -> Result<()> {
    let mut client = TypedConvexClient::new(deployment_url)
        .await
        .context("failed to connect typed Convex client")?;
    let messages = client
        .query(messages::collect(), &())
        .await
        .context("typed collect query failed")?;

    print_messages("messages", &messages);
    Ok(())
}

async fn find_messages(deployment_url: &str, author: String) -> Result<()> {
    let mut client = TypedConvexClient::new(deployment_url)
        .await
        .context("failed to connect typed Convex client")?;
    let messages = client
        .query(messages::find_by_author(), &FindByAuthorArgs { author })
        .await
        .context("typed findByAuthor query failed")?;

    print_messages("matching messages", &messages);
    Ok(())
}

async fn show_status(deployment_url: &str) -> Result<()> {
    let mut client = TypedConvexClient::new(deployment_url)
        .await
        .context("failed to connect typed Convex client")?;
    let response = client
        .query(messages::multi_return_demo(), &())
        .await
        .context("typed multiReturnDemo query failed")?;

    match response {
        MultiReturnDemoResponse::MessagesCountError {
            messages,
            count,
            error,
        } => {
            println!("count: {count}");
            println!("error: {error:?}");
            print_messages("messages", &messages);
        }
        MultiReturnDemoResponse::Error { error } => {
            println!("error: {error}");
        }
    }

    Ok(())
}

async fn watch_messages(deployment_url: &str, updates: usize) -> Result<()> {
    let mut raw_client = ConvexClient::new(deployment_url)
        .await
        .context("failed to connect raw Convex client")?;
    let mut subscription = raw_client
        .subscribe(<Collect as FunctionSpec>::PATH, encode_args(&())?)
        .await
        .context("failed to subscribe to messages:collect")?;

    for index in 1..=updates {
        let snapshot = subscription
            .next()
            .await
            .context("subscription ended before delivering the requested updates")?;
        let messages: CollectResponse =
            decode_result(snapshot).context("failed to decode typed collect subscription")?;
        println!("update #{index}");
        print_messages("messages", &messages);
    }

    Ok(())
}

fn print_inserted_id(id: &MessagesId) {
    println!("inserted message id: {}", id.0);
}

fn print_messages<T>(label: &str, messages: &[T])
where
    T: MessageLike,
{
    println!("{label}: {}", messages.len());
    for message in messages {
        println!(
            "- {} [{}] {}",
            message.id(),
            message.creation_time(),
            format!("{}: {}", message.author(), message.body())
        );
    }
}

trait MessageLike {
    fn id(&self) -> &str;
    fn creation_time(&self) -> f64;
    fn author(&self) -> &str;
    fn body(&self) -> &str;
}

impl MessageLike for messages::CollectResponseItem {
    fn id(&self) -> &str {
        &self.id.0
    }

    fn creation_time(&self) -> f64 {
        self.creation_time
    }

    fn author(&self) -> &str {
        &self.author
    }

    fn body(&self) -> &str {
        &self.body
    }
}

impl MessageLike for messages::FindByAuthorResponseItem {
    fn id(&self) -> &str {
        &self.id.0
    }

    fn creation_time(&self) -> f64 {
        self.creation_time
    }

    fn author(&self) -> &str {
        &self.author
    }

    fn body(&self) -> &str {
        &self.body
    }
}

impl MessageLike for messages::MultiReturnDemoResponseVariant1MessagesItem {
    fn id(&self) -> &str {
        &self.id.0
    }

    fn creation_time(&self) -> f64 {
        self.creation_time
    }

    fn author(&self) -> &str {
        &self.author
    }

    fn body(&self) -> &str {
        &self.body
    }
}
