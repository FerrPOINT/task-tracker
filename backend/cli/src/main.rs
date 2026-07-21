use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tt", version, about = "Task Tracker CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check CLI connectivity
    Ping,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    println!("Task Tracker CLI ready");
    Ok(())
}
