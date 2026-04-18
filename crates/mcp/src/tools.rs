use dm_core::Core;
use rmcp::{
    model::{Implementation, ServerInfo},
    ServerHandler,
};

#[derive(Clone)]
pub struct MemoryServer {
    pub core: Core,
}

impl MemoryServer {
    pub fn new(core: Core) -> Self {
        Self { core }
    }
}

impl ServerHandler for MemoryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "directive-memory".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some(
                "Directive Memory - BM25 search over markdown files plus write-back.".into(),
            ),
            ..Default::default()
        }
    }
}
