//! Directive Memory MCP server (stdio transport).

pub mod tools;

use anyhow::Result;
use dm_core::Core;
use rmcp::{transport::stdio, ServiceExt};
use tools::MemoryServer;

pub async fn run_stdio(core: Core) -> Result<()> {
    let server = MemoryServer::new(core);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
