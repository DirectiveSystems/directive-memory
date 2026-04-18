use dm_core::{search::SearchQuery, Core};
use rmcp::{
    model::{Implementation, ServerInfo},
    schemars, tool, ServerHandler,
};
use serde::Deserialize;

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchArgs {
    /// Search query text
    pub query: String,
    /// Number of results to return
    #[serde(default = "default_top_k")]
    pub top_k: i64,
    /// Only return results from files matching this prefix
    #[serde(default)]
    pub filter_file: String,
    /// Only return results with this source type (memory|project|vault|contact)
    #[serde(default)]
    pub filter_source_type: String,
}
fn default_top_k() -> i64 {
    5
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WriteMemoryArgs {
    /// Relative path within memory_dir (e.g., "projects/sift.md")
    pub file_path: String,
    /// Markdown content to write
    pub content: String,
    /// If true, append. If false (default), overwrite.
    #[serde(default)]
    pub append: bool,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AddFactArgs {
    /// Relative path within memory_dir (e.g., "learnings.md")
    pub file_path: String,
    /// Section heading (e.g., "## Patterns")
    pub section: String,
    /// Fact to add (formatted as "- <fact>")
    pub fact: String,
}

#[derive(Clone)]
pub struct MemoryServer {
    pub core: Core,
}

impl MemoryServer {
    pub fn new(core: Core) -> Self {
        Self { core }
    }
}

#[tool(tool_box)]
impl MemoryServer {
    #[tool(description = "BM25 search over memory files")]
    pub async fn search_memory(&self, #[tool(aggr)] args: SearchArgs) -> String {
        let q = SearchQuery {
            query: args.query.clone(),
            top_k: args.top_k,
            filter_file: args.filter_file,
            filter_source_type: if args.filter_source_type.is_empty() {
                None
            } else {
                Some(args.filter_source_type)
            },
        };
        let hits = match self.core.search(&q).await {
            Ok(h) => h,
            Err(e) => return format!("error: {e}"),
        };
        if hits.is_empty() {
            return format!("No results for '{}'.", args.query);
        }
        let mut out = format!("{} results for '{}':\n\n", hits.len(), args.query);
        for h in hits {
            out.push_str(&format!(
                "[{}] {}\n  {}\n\n",
                h.file,
                h.heading,
                h.content.chars().take(200).collect::<String>()
            ));
        }
        out
    }

    #[tool(description = "List indexed memory files")]
    pub async fn list_memory_files(&self) -> String {
        match self.core.list_files().await {
            Ok(files) if files.is_empty() => "No memory files indexed.".into(),
            Ok(files) => files
                .into_iter()
                .map(|(p, _)| format!("- {p}"))
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(description = "Get index statistics")]
    pub async fn get_memory_stats(&self) -> String {
        match self.core.stats().await {
            Ok(s) => {
                let mut out = format!(
                    "Chunks: {}\nFiles: {}\nSearch log: {} total, {} last 7 days\n\nSource types:\n",
                    s.chunks, s.files, s.search_log_total, s.search_log_last_7d
                );
                for (k, v) in s.source_types {
                    out.push_str(&format!("  {k}: {v}\n"));
                }
                out
            }
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(description = "Write or append to a memory file")]
    pub async fn write_memory(&self, #[tool(aggr)] args: WriteMemoryArgs) -> String {
        match self.core.write_file(&args.file_path, &args.content, args.append).await {
            Ok(()) => format!(
                "{} {}",
                if args.append { "Appended to" } else { "Wrote" },
                args.file_path
            ),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(description = "Add a fact as a bullet under a section in a memory file")]
    pub async fn add_fact(&self, #[tool(aggr)] args: AddFactArgs) -> String {
        match self.core.add_fact(&args.file_path, &args.section, &args.fact).await {
            Ok(()) => format!("Added fact to {} under '{}'", args.file_path, args.section),
            Err(e) => format!("error: {e}"),
        }
    }
}

#[tool(tool_box)]
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
