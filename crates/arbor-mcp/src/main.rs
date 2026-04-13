use anyhow::{Context, Result};
use rmcp::ServiceExt;
use std::path::PathBuf;
use tokio::io::{stdin, stdout};

mod tools;
use tools::ArborServer;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    let cli_mode = args.iter().any(|a| a == "--cli");
    let compact_mode = args.iter().any(|a| a == "--compact");
    let languages_mode = args.iter().any(|a| a == "--languages");

    if languages_mode {
        let analyzer = arbor_analyzers::code::CodeAnalyzer::new();
        println!("{}", analyzer.language_features_markdown());
        return Ok(());
    }

    let root = args
        .iter()
        .find(|a| !a.starts_with('-') && *a != &args[0])
        .map_or_else(
            || std::env::current_dir().context("failed to determine working directory"),
            |p| Ok(PathBuf::from(p)),
        )?;

    let root = std::fs::canonicalize(&root)?;

    eprintln!("Arbor: indexing {}...", root.display());
    let server = ArborServer::new(root)?;

    if cli_mode || compact_mode {
        let boot = server.boot_cli();
        println!("{boot}");
        if compact_mode {
            println!("{}", server.compact_cli());
        } else {
            println!("{}", server.skeleton_cli());
        }
        return Ok(());
    }

    eprintln!("Arbor: ready, starting MCP server on stdio");
    let transport = (stdin(), stdout());
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
