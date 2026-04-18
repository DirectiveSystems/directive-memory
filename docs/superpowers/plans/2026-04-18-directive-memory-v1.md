# Directive Memory v1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a standalone Rust binary that indexes markdown files into SQLite FTS5, exposes hybrid search + write-back through REST, MCP (stdio), and a minimal web UI — the open-source core of Directive Memory.

**Architecture:** A Cargo workspace with three library crates (`dm-core`, `dm-api`, `dm-mcp`) and a top-level `directive-memory` binary that composes them behind a `clap` subcommand interface. `dm-core` owns the SQLite schema (via `sqlx`), markdown chunker, BM25 search with temporal decay, and safe file write-back. `dm-api` wraps core behind an `axum` router with API-key auth and serves the web UI as static files. `dm-mcp` wraps core as MCP tools over stdio using `rmcp`. Vector search / cross-encoder / MMR are explicitly deferred to phase 2 — BM25 + temporal decay is the v1 ranking pipeline.

**Tech Stack:** Rust edition 2021 (MSRV 1.75), `sqlx` (SQLite + FTS5), `axum` 0.7, `tower-http`, `tokio`, `rmcp` (MCP SDK), `clap` (CLI), `serde`, `walkdir`, regex-based chunker (mirrors the proven Python impl), `config` + `toml`.

**Non-goals (per spec):** Vector embeddings, cross-encoder reranking, MMR diversity, PostgreSQL backend, real-time file watching, Obsidian plugin, conversation indexing. The schema carves out room for vectors (future `chunk_vecs` virtual table) but no vector code ships in v1.

---

## File Structure

```
directive-memory/
├── Cargo.toml                       # workspace manifest + release profile
├── rust-toolchain.toml              # pin to stable
├── .gitignore
├── LICENSE                          # AGPLv3 full text
├── README.md                        # overview, quickstart, MCP setup
├── config.example.toml              # documented config template
├── migrations/
│   └── 20260418000001_initial.sql   # files, chunks FTS5, chunk_map, search_log, meta
├── web/
│   ├── index.html                   # search UI shell
│   ├── styles.css                   # minimal styling
│   └── app.js                       # fetch → render results (DOM-safe)
├── src/
│   └── main.rs                      # clap subcommands: serve | mcp | reindex | search
└── crates/
    ├── core/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs               # re-exports: config, db, indexer, search, writeback, stats
    │       ├── config.rs            # Config struct + toml/env loader
    │       ├── db.rs                # sqlx SqlitePool + migration runner
    │       ├── chunker.rs           # parse_chunks + split_by_paragraphs
    │       ├── source_type.rs       # path prefix → SourceType
    │       ├── indexer.rs           # walk dirs, diff mtime, upsert chunks
    │       ├── search.rs            # BM25 query, filters, temporal decay, search_log
    │       ├── writeback.rs         # path validation, write_file, append_file, add_fact
    │       ├── stats.rs             # counts + source breakdown + log stats
    │       ├── core.rs              # Core facade
    │       └── error.rs             # CoreError enum (thiserror)
    ├── api/
    │   ├── Cargo.toml
    │   └── src/
    │       ├── lib.rs               # build_router(state) -> axum::Router
    │       ├── state.rs             # AppState { core }
    │       ├── auth.rs              # require_api_key middleware
    │       ├── error.rs             # ApiError → IntoResponse
    │       └── routes/
    │           ├── mod.rs
    │           ├── search.rs        # GET /api/search
    │           ├── files.rs         # GET /api/files, GET|POST|PATCH /api/files/*path
    │           ├── facts.rs         # POST /api/facts
    │           ├── stats.rs         # GET /api/stats, POST /api/reindex
    │           └── static_ui.rs     # serves web/ at /
    └── mcp/
        ├── Cargo.toml
        └── src/
            ├── lib.rs               # run_stdio(core) -> Result<()>
            └── tools.rs             # 5 MCP tools wrapping Core
```

**Responsibility boundaries:**
- `dm-core` is the single source of truth for domain logic; it exposes a `Core` struct that both API and MCP wrap.
- `dm-api` and `dm-mcp` have *no* direct SQL — all DB access goes through `Core`.
- The top-level binary only parses CLI flags, loads config, builds a `Core`, and dispatches to the chosen surface.

---

## Task 1: Workspace skeleton + license + gitignore

**Files:**
- Create: `/home/jeeves/directive-memory/Cargo.toml`
- Create: `/home/jeeves/directive-memory/rust-toolchain.toml`
- Create: `/home/jeeves/directive-memory/.gitignore`
- Create: `/home/jeeves/directive-memory/LICENSE`
- Create: `/home/jeeves/directive-memory/README.md` (stub; filled in Task 23)
- Create: `/home/jeeves/directive-memory/crates/{core,api,mcp}/Cargo.toml` and `src/lib.rs`
- Create: `/home/jeeves/directive-memory/src/main.rs`

- [ ] **Step 1: Write `rust-toolchain.toml`**

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Write `.gitignore`**

```gitignore
/target
/data
*.db
*.db-journal
*.db-wal
*.db-shm
.env
.DS_Store
```

- [ ] **Step 3: Write `LICENSE`** — fetch AGPLv3 text from https://www.gnu.org/licenses/agpl-3.0.txt via WebFetch and write verbatim.

- [ ] **Step 4: Write workspace `Cargo.toml`**

```toml
[workspace]
resolver = "2"
members = ["crates/core", "crates/api", "crates/mcp"]
default-members = ["."]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "AGPL-3.0-or-later"
repository = "https://github.com/directive-systems/directive-memory"
rust-version = "1.75"

[workspace.dependencies]
tokio = { version = "1.38", features = ["rt-multi-thread", "macros", "fs", "io-util", "signal"] }
sqlx = { version = "0.8", default-features = false, features = ["runtime-tokio", "sqlite", "macros", "chrono", "migrate"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
walkdir = "2"
regex = "1"
once_cell = "1"
config = { version = "0.14", default-features = false, features = ["toml"] }
toml = "0.8"

dm-core = { path = "crates/core" }
dm-api  = { path = "crates/api"  }
dm-mcp  = { path = "crates/mcp"  }

[package]
name = "directive-memory"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[[bin]]
name = "directive-memory"
path = "src/main.rs"

[dependencies]
dm-core.workspace = true
dm-api.workspace = true
dm-mcp.workspace = true
tokio.workspace = true
anyhow.workspace = true
tracing.workspace = true
tracing-subscriber.workspace = true
clap = { version = "4", features = ["derive"] }

[profile.release]
lto = "fat"
codegen-units = 1
strip = "symbols"
opt-level = 3
panic = "abort"
```

- [ ] **Step 5: Write `crates/core/Cargo.toml`**

```toml
[package]
name = "dm-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
tokio.workspace = true
sqlx.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
anyhow.workspace = true
chrono.workspace = true
tracing.workspace = true
walkdir.workspace = true
regex.workspace = true
once_cell.workspace = true
config.workspace = true
toml.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["rt-multi-thread", "macros", "fs", "io-util", "signal", "test-util"] }
tempfile = "3"
```

- [ ] **Step 6: Write `crates/api/Cargo.toml`**

```toml
[package]
name = "dm-api"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
dm-core.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
anyhow.workspace = true
chrono.workspace = true
tracing.workspace = true
axum = { version = "0.7", features = ["macros"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "cors"] }
mime_guess = "2"
rust-embed = "8"

[dev-dependencies]
tempfile = "3"
tower = { version = "0.4", features = ["util"] }
http-body-util = "0.1"
```

- [ ] **Step 7: Write `crates/mcp/Cargo.toml`**

```toml
[package]
name = "dm-mcp"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
dm-core.workspace = true
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
anyhow.workspace = true
thiserror.workspace = true
tracing.workspace = true
rmcp = { version = "0.1", features = ["server", "transport-io"] }
schemars = "0.8"
```

> Note: `rmcp` is the official Model Context Protocol Rust SDK. If the version on crates.io differs from what's documented here at execution time, pick the latest `0.x` and adjust. If `rmcp` proves unworkable, fall back to the hand-rolled JSON-RPC path in Appendix A.

- [ ] **Step 8: Write stub crate libs**

`crates/core/src/lib.rs`:
```rust
//! Directive Memory core: domain, search, and storage.
```

`crates/api/src/lib.rs`:
```rust
//! Directive Memory HTTP API.
```

`crates/mcp/src/lib.rs`:
```rust
//! Directive Memory MCP server.
```

- [ ] **Step 9: Write binary stub `src/main.rs`**

```rust
fn main() {
    println!("directive-memory: skeleton");
}
```

- [ ] **Step 10: Write stub `README.md`**

```markdown
# Directive Memory

An open-source, MCP-native personal knowledge base. Markdown in, hybrid search + AI
writes out.

Status: pre-release — see `docs/superpowers/plans/` for the implementation plan.

Licensed under AGPLv3.
```

- [ ] **Step 11: Verify the workspace builds**

Run: `cargo build --workspace`
Expected: successful build with warnings only about unused code.

- [ ] **Step 12: Commit**

```bash
git add .gitignore LICENSE README.md Cargo.toml rust-toolchain.toml crates src
git commit -m "feat: initialize directive-memory Rust workspace"
```

---

## Task 2: Database layer — migrations + connection pool

**Files:**
- Create: `/home/jeeves/directive-memory/migrations/20260418000001_initial.sql`
- Create: `/home/jeeves/directive-memory/crates/core/src/error.rs`
- Create: `/home/jeeves/directive-memory/crates/core/src/db.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/core/tests/db.rs`

- [ ] **Step 1: Write the migration SQL**

```sql
-- migrations/20260418000001_initial.sql

-- Tracks indexed markdown files for incremental reindex.
CREATE TABLE files (
    path  TEXT PRIMARY KEY,
    mtime REAL NOT NULL
);
-- No explicit idx_files_path: the PRIMARY KEY already provides a unique B-tree.

-- Full-text search index (BM25 ranking built in).
-- Duplicates file/heading/content with chunk_map; keeping FTS5 in "contentless"
-- mirror mode is phase-2 polish. Both tables are written in one transaction
-- from the indexer, so divergence is bounded.
CREATE VIRTUAL TABLE chunks USING fts5(
    file, heading, content
);

-- Canonical chunk storage with metadata.
-- tags: comma-separated (format stabilises in a later task).
CREATE TABLE chunk_map (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    file          TEXT NOT NULL,
    heading       TEXT NOT NULL,
    content       TEXT NOT NULL,
    source_type   TEXT NOT NULL DEFAULT 'memory',
    tags          TEXT NOT NULL DEFAULT '',
    importance    REAL NOT NULL DEFAULT 0.0,
    access_count  INTEGER NOT NULL DEFAULT 0,
    last_accessed TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_chunk_map_file          ON chunk_map(file);
CREATE INDEX idx_chunk_map_source_type   ON chunk_map(source_type);
-- Composite index for the (file, heading) lookup pattern in search.rs
-- after BM25 hits resolve to source_type / metadata.
CREATE INDEX idx_chunk_map_file_heading  ON chunk_map(file, heading);

-- Search telemetry. Unbounded by design in v1; a retention cron is phase-2.
CREATE TABLE search_log (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    ts           TEXT NOT NULL,
    query        TEXT NOT NULL,
    mode         TEXT NOT NULL,
    top_k        INTEGER NOT NULL,
    result_count INTEGER NOT NULL,
    top_results  TEXT NOT NULL
);
CREATE INDEX idx_search_log_ts ON search_log(ts);

-- Model metadata (reserved for phase-2 vector drift detection).
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

- [ ] **Step 2: Write the failing test**

```rust
// crates/core/tests/db.rs
use dm_core::db;
use tempfile::tempdir;

#[tokio::test]
async fn open_creates_file_and_runs_migrations() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test.db");
    let pool = db::open(&db_path).await.expect("open pool");
    for name in ["files", "chunks", "chunk_map", "search_log", "meta"] {
        let row: (String,) = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name = ?1"
        )
        .bind(name).fetch_one(&pool).await
        .unwrap_or_else(|e| panic!("table {name} missing: {e}"));
        assert_eq!(row.0, name);
    }
    assert!(db_path.exists());
}
```

- [ ] **Step 3: Run the test; confirm failure**

Run: `cargo test -p dm-core --test db`
Expected: compile error — module `db` not found.

- [ ] **Step 4: Write `crates/core/src/error.rs`**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
```

- [ ] **Step 5: Write `crates/core/src/db.rs`**

```rust
use crate::error::{CoreError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::Path;
use std::str::FromStr;

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn open(path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let url = format!("sqlite://{}", path.display());
    let opts = SqliteConnectOptions::from_str(&url)
        .map_err(|e| CoreError::Other(format!("bad sqlite url: {e}")))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(30));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts).await?;
    MIGRATOR.run(&pool).await?;
    Ok(pool)
}
```

- [ ] **Step 6: Wire modules into `lib.rs`**

```rust
//! Directive Memory core.
pub mod db;
pub mod error;
pub use error::{CoreError, Result};
```

- [ ] **Step 7: Make migrations reachable from the core crate**

`sqlx::migrate!("./migrations")` resolves paths relative to the crate's
`CARGO_MANIFEST_DIR`. The canonical migrations dir lives at the workspace
root, so add a symlink inside the core crate:

```bash
ln -s ../../migrations crates/core/migrations
```

(Windows fallback: change the macro to `sqlx::migrate!("../../migrations")`.)

- [ ] **Step 8: Run the test and confirm pass**

Run: `cargo test -p dm-core --test db`
Expected: `test open_creates_file_and_runs_migrations ... ok`

- [ ] **Step 9: Commit**

```bash
git add migrations crates/core
git commit -m "feat(core): add SQLite schema and connection pool"
```

---

## Task 3: Markdown chunker

**Files:**
- Create: `/home/jeeves/directive-memory/crates/core/src/chunker.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/src/lib.rs`

Ports the Python `_parse_chunks` / `_split_by_paragraphs` logic. Max chunk size 800 chars; splits on h1-h4 headings; long sections further split by paragraph groups.

- [ ] **Step 1: Write failing tests (in the same `chunker.rs` as a `#[cfg(test)] mod tests`)**

```rust
// crates/core/src/chunker.rs
use once_cell::sync::Lazy;
use regex::Regex;

pub const MAX_CHUNK_CHARS: usize = 800;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub heading: String,
    pub content: String,
}

pub fn parse_chunks(_text: &str) -> Vec<Chunk> {
    unimplemented!()
}

pub fn split_by_paragraphs(_text: &str, _max_chars: usize) -> Vec<String> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_headings() {
        let md = "# Top\nintro\n\n## One\nalpha\n\n## Two\nbeta\n";
        let chunks = parse_chunks(md);
        let headings: Vec<&str> = chunks.iter().map(|c| c.heading.as_str()).collect();
        assert_eq!(headings, vec!["Top", "One", "Two"]);
    }

    #[test]
    fn preamble_before_heading_uses_top_heading() {
        let chunks = parse_chunks("just some text\nno heading\n");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "(top)");
        assert!(chunks[0].content.contains("just some text"));
    }

    #[test]
    fn drops_empty_sections() {
        let chunks = parse_chunks("# Empty\n\n## Real\ncontent\n");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "Real");
    }

    #[test]
    fn large_section_is_subchunked_with_numbered_headings() {
        let para = "sentence. ".repeat(100); // ~1000 chars
        let md = format!("# Big\n\n{para}\n\n{para}\n");
        let chunks = parse_chunks(&md);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].heading.starts_with("Big ("));
        assert!(chunks.iter().all(|c| c.content.len() <= MAX_CHUNK_CHARS * 2));
    }

    #[test]
    fn short_section_below_limit_is_single_chunk() {
        let chunks = parse_chunks("# Small\nshort content here\n");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].heading, "Small");
    }
}
```

And register in `lib.rs`:

```rust
pub mod chunker;
```

- [ ] **Step 2: Run the failing tests**

Run: `cargo test -p dm-core chunker::tests`
Expected: all 5 tests panic at `unimplemented!`.

- [ ] **Step 3: Implement both functions**

Replace the stubs in `chunker.rs`:

```rust
static HEADING_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^#{1,4}\s+").unwrap());

pub fn parse_chunks(text: &str) -> Vec<Chunk> {
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_heading = String::from("(top)");
    let mut current_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        if HEADING_RE.is_match(line) {
            if !current_lines.is_empty() {
                sections.push((current_heading.clone(), current_lines.join("\n").trim().to_string()));
            }
            current_heading = HEADING_RE.replace(line, "").trim().to_string();
            current_lines.clear();
        } else {
            current_lines.push(line);
        }
    }
    if !current_lines.is_empty() {
        sections.push((current_heading, current_lines.join("\n").trim().to_string()));
    }

    let mut out: Vec<Chunk> = Vec::new();
    for (heading, content) in sections {
        if content.is_empty() { continue; }
        if content.len() <= MAX_CHUNK_CHARS {
            out.push(Chunk { heading, content });
            continue;
        }
        let subs = split_by_paragraphs(&content, MAX_CHUNK_CHARS);
        if subs.len() == 1 {
            out.push(Chunk { heading, content });
        } else {
            for (i, sc) in subs.into_iter().enumerate() {
                out.push(Chunk { heading: format!("{heading} ({})", i + 1), content: sc });
            }
        }
    }
    out
}

pub fn split_by_paragraphs(text: &str, max_chars: usize) -> Vec<String> {
    let mut paragraphs: Vec<Vec<&str>> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() && !current.is_empty() {
            paragraphs.push(std::mem::take(&mut current));
        } else if !line.trim().is_empty() {
            current.push(line);
        }
    }
    if !current.is_empty() { paragraphs.push(current); }

    let mut chunks: Vec<String> = Vec::new();
    let mut buf: Vec<&str> = Vec::new();
    let mut buf_chars: usize = 0;
    let para_chars = |p: &[&str]| -> usize { p.iter().map(|l| l.len() + 1).sum() };

    for para in paragraphs {
        let pc = para_chars(&para);
        if !buf.is_empty() && buf_chars + pc > max_chars {
            chunks.push(buf.join("\n").trim().to_string());
            buf.clear();
            buf_chars = 0;
        }
        buf.extend(&para);
        buf_chars += pc;
        while buf_chars > max_chars * 2 {
            let mut taken: Vec<&str> = Vec::new();
            let mut taken_chars: usize = 0;
            while !buf.is_empty() && taken_chars < max_chars {
                let line = buf.remove(0);
                taken_chars += line.len() + 1;
                taken.push(line);
            }
            chunks.push(taken.join("\n").trim().to_string());
            buf_chars = buf.iter().map(|l| l.len() + 1).sum();
        }
    }
    if !buf.is_empty() { chunks.push(buf.join("\n").trim().to_string()); }
    chunks.into_iter().filter(|c| !c.is_empty()).collect()
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p dm-core chunker::tests`
Expected: `5 passed; 0 failed`.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/chunker.rs crates/core/src/lib.rs
git commit -m "feat(core): markdown chunker (headings + paragraph splits)"
```

---

## Task 4: Source type classifier

**Files:**
- Create: `/home/jeeves/directive-memory/crates/core/src/source_type.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/src/lib.rs`

- [ ] **Step 1: Write the module with tests and implementation inline**

```rust
// crates/core/src/source_type.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType { Memory, Project, Vault, Contact }

impl SourceType {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceType::Memory  => "memory",
            SourceType::Project => "project",
            SourceType::Vault   => "vault",
            SourceType::Contact => "contact",
        }
    }
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "memory"  => Some(SourceType::Memory),
            "project" => Some(SourceType::Project),
            "vault"   => Some(SourceType::Vault),
            "contact" => Some(SourceType::Contact),
            _ => None,
        }
    }
}

pub fn infer(rel_path: &str) -> SourceType {
    if rel_path.starts_with("vault/")    { return SourceType::Vault; }
    if rel_path.starts_with("projects/") { return SourceType::Project; }
    if rel_path.starts_with("contacts/") { return SourceType::Contact; }
    SourceType::Memory
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefix_rules() {
        assert_eq!(infer("note.md"), SourceType::Memory);
        assert_eq!(infer("vault/daily/2026-04-18.md"), SourceType::Vault);
        assert_eq!(infer("projects/foo.md"), SourceType::Project);
        assert_eq!(infer("contacts/holly.md"), SourceType::Contact);
    }
    #[test]
    fn roundtrip_str() {
        for st in [SourceType::Memory, SourceType::Project, SourceType::Vault, SourceType::Contact] {
            assert_eq!(SourceType::from_str(st.as_str()), Some(st));
        }
        assert!(SourceType::from_str("bogus").is_none());
    }
}
```

Register in `lib.rs`:

```rust
pub mod source_type;
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p dm-core source_type::tests`
Expected: `2 passed`.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/source_type.rs crates/core/src/lib.rs
git commit -m "feat(core): classify source type from file path prefix"
```

---

## Task 5: File indexer

**Files:**
- Create: `/home/jeeves/directive-memory/crates/core/src/indexer.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/core/tests/indexer.rs`

Walks a set of (dir, prefix) roots, diffs mtimes against `files`, and upserts chunks into `chunk_map` + FTS5 `chunks`. Prunes entries for disappeared files.

- [ ] **Step 1: Write failing tests**

```rust
// crates/core/tests/indexer.rs
use dm_core::{db, indexer::{self, IndexRoot}};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn indexes_new_files_and_detects_mtime_changes() {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("alpha.md"), "# Alpha\ncontent about alpha").unwrap();
    fs::write(mem.join("beta.md"),  "# Beta\ncontent about beta").unwrap();

    let pool = db::open(&dir.path().join("idx.db")).await.unwrap();
    let roots = vec![IndexRoot { dir: mem.clone(), prefix: String::new() }];

    let report = indexer::reindex(&pool, &roots).await.unwrap();
    assert_eq!(report.files_indexed, 2);
    assert_eq!(report.files_pruned, 0);

    let noop = indexer::reindex(&pool, &roots).await.unwrap();
    assert_eq!(noop.files_indexed, 0);

    std::thread::sleep(std::time::Duration::from_millis(1100));
    fs::write(mem.join("alpha.md"), "# Alpha\nbrand new content").unwrap();
    let touched = indexer::reindex(&pool, &roots).await.unwrap();
    assert_eq!(touched.files_indexed, 1);

    fs::remove_file(mem.join("beta.md")).unwrap();
    let pruned = indexer::reindex(&pool, &roots).await.unwrap();
    assert_eq!(pruned.files_pruned, 1);
}

#[tokio::test]
async fn applies_prefix_for_external_roots() {
    let dir = tempdir().unwrap();
    let vault = dir.path().join("vault_src");
    fs::create_dir_all(&vault).unwrap();
    fs::write(vault.join("note.md"), "# Note\ntext").unwrap();

    let pool = db::open(&dir.path().join("idx.db")).await.unwrap();
    let roots = vec![IndexRoot { dir: vault.clone(), prefix: "vault/".into() }];
    indexer::reindex(&pool, &roots).await.unwrap();

    let (path,): (String,) = sqlx::query_as("SELECT path FROM files LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(path, "vault/note.md");

    let (source_type,): (String,) = sqlx::query_as("SELECT source_type FROM chunk_map LIMIT 1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(source_type, "vault");
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p dm-core --test indexer`
Expected: compile error — `indexer` module missing.

- [ ] **Step 3: Implement `crates/core/src/indexer.rs`**

```rust
use crate::{chunker, error::Result, source_type};
use sqlx::SqlitePool;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct IndexRoot {
    pub dir: PathBuf,
    /// Virtual prefix applied to relative paths (e.g. "vault/") — empty for the primary root.
    pub prefix: String,
}

#[derive(Debug, Default)]
pub struct IndexReport {
    pub files_indexed: usize,
    pub files_pruned: usize,
}

pub async fn reindex(pool: &SqlitePool, roots: &[IndexRoot]) -> Result<IndexReport> {
    let mut report = IndexReport::default();
    let mut live: HashSet<String> = HashSet::new();

    for root in roots {
        if !root.dir.exists() { continue; }
        for entry in WalkDir::new(&root.dir).into_iter().filter_map(|e| e.ok()) {
            if !entry.file_type().is_file() { continue; }
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") { continue; }

            let rel_os = path.strip_prefix(&root.dir).unwrap();
            let rel = format!("{}{}", root.prefix, rel_os.to_string_lossy().replace('\\', "/"));
            live.insert(rel.clone());

            let mtime = mtime_of(path)?;
            if needs_reindex(pool, &rel, mtime).await? {
                index_file(pool, path, &rel, mtime).await?;
                report.files_indexed += 1;
            }
        }
    }

    let indexed: Vec<(String,)> = sqlx::query_as("SELECT path FROM files")
        .fetch_all(pool).await?;
    for (path,) in indexed {
        if !live.contains(&path) {
            delete_file(pool, &path).await?;
            report.files_pruned += 1;
        }
    }
    Ok(report)
}

fn mtime_of(path: &Path) -> Result<f64> {
    let meta = std::fs::metadata(path)?;
    let mtime = meta.modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| crate::error::CoreError::Other(e.to_string()))?
        .as_secs_f64();
    Ok(mtime)
}

async fn needs_reindex(pool: &SqlitePool, rel: &str, mtime: f64) -> Result<bool> {
    let row: Option<(f64,)> = sqlx::query_as("SELECT mtime FROM files WHERE path = ?1")
        .bind(rel).fetch_optional(pool).await?;
    Ok(match row { None => true, Some((old,)) => old < mtime })
}

async fn index_file(pool: &SqlitePool, path: &Path, rel: &str, mtime: f64) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let chunks = chunker::parse_chunks(&text);
    let st = source_type::infer(rel).as_str();

    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM chunks WHERE file = ?1").bind(rel).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM chunk_map WHERE file = ?1").bind(rel).execute(&mut *tx).await?;
    sqlx::query("INSERT OR REPLACE INTO files (path, mtime) VALUES (?1, ?2)")
        .bind(rel).bind(mtime).execute(&mut *tx).await?;
    for c in &chunks {
        sqlx::query("INSERT INTO chunks (file, heading, content) VALUES (?1, ?2, ?3)")
            .bind(rel).bind(&c.heading).bind(&c.content).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO chunk_map (file, heading, content, source_type) VALUES (?1, ?2, ?3, ?4)"
        )
        .bind(rel).bind(&c.heading).bind(&c.content).bind(st)
        .execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn delete_file(pool: &SqlitePool, rel: &str) -> Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM chunks WHERE file = ?1").bind(rel).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM chunk_map WHERE file = ?1").bind(rel).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM files WHERE path = ?1").bind(rel).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
```

Register in `lib.rs`:

```rust
pub mod indexer;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p dm-core --test indexer`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/indexer.rs crates/core/src/lib.rs crates/core/tests/indexer.rs
git commit -m "feat(core): index markdown files with mtime diffing and pruning"
```

---

## Task 6: BM25 search with filters

**Files:**
- Create: `/home/jeeves/directive-memory/crates/core/src/search.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/core/tests/search.rs`

- [ ] **Step 1: Write failing tests**

```rust
// crates/core/tests/search.rs
use dm_core::{db, indexer::{self, IndexRoot}, search::{self, SearchQuery}};
use std::fs;
use tempfile::tempdir;

async fn setup() -> (tempfile::TempDir, sqlx::SqlitePool) {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("alpha.md"), "# Alpha\nfencing quote was 3638 dollars").unwrap();
    fs::write(mem.join("beta.md"),  "# Beta\nsomething else entirely").unwrap();

    let projects = mem.join("projects");
    fs::create_dir_all(&projects).unwrap();
    fs::write(projects.join("sift.md"), "# Sift\nfencing discussion").unwrap();

    let pool = db::open(&dir.path().join("s.db")).await.unwrap();
    indexer::reindex(&pool, &[IndexRoot { dir: mem, prefix: String::new() }]).await.unwrap();
    (dir, pool)
}

#[tokio::test]
async fn bm25_finds_matching_chunks() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing".into(), top_k: 5, ..Default::default()
    }).await.unwrap();
    assert!(!hits.is_empty());
    assert!(hits.iter().any(|h| h.file == "alpha.md"));
}

#[tokio::test]
async fn filter_by_file_prefix() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing".into(), top_k: 5,
        filter_file: "projects/".into(), ..Default::default()
    }).await.unwrap();
    assert!(!hits.is_empty());
    assert!(hits.iter().all(|h| h.file.starts_with("projects/")));
}

#[tokio::test]
async fn filter_by_source_type() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing".into(), top_k: 5,
        filter_source_type: Some("project".into()), ..Default::default()
    }).await.unwrap();
    assert!(hits.iter().all(|h| h.file.starts_with("projects/")));
}

#[tokio::test]
async fn sanitizes_punctuation_in_query() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing? \"quote\"".into(), top_k: 5, ..Default::default()
    }).await.unwrap();
    assert!(!hits.is_empty());
}

#[tokio::test]
async fn empty_query_returns_empty() {
    let (_d, pool) = setup().await;
    let hits = search::search(&pool, &SearchQuery {
        query: "   ".into(), top_k: 5, ..Default::default()
    }).await.unwrap();
    assert!(hits.is_empty());
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p dm-core --test search`
Expected: compile error — `search` module missing.

- [ ] **Step 3: Implement `crates/core/src/search.rs`**

```rust
use crate::error::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use sqlx::SqlitePool;

static NON_WORD_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\w\s]").unwrap());

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub query: String,
    pub top_k: i64,
    pub filter_file: String,
    pub filter_source_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub file: String,
    pub heading: String,
    pub content: String,
    pub score: f64,
    pub source_type: String,
}

pub async fn search(pool: &SqlitePool, q: &SearchQuery) -> Result<Vec<SearchHit>> {
    let sanitized = NON_WORD_RE.replace_all(&q.query, " ").trim().to_string();
    if sanitized.is_empty() { return Ok(Vec::new()); }

    let top_k = q.top_k.max(1);
    let has_filters = !q.filter_file.is_empty() || q.filter_source_type.is_some();
    let pool_size = if has_filters { top_k * 4 } else { top_k };

    let rows: Vec<(String, String, String, f64)> = sqlx::query_as(
        "SELECT c.file, c.heading, c.content, rank \
         FROM chunks c WHERE chunks MATCH ?1 \
         ORDER BY rank LIMIT ?2"
    )
    .bind(&sanitized).bind(pool_size)
    .fetch_all(pool).await?;

    let mut hits: Vec<SearchHit> = Vec::with_capacity(rows.len());
    for (file, heading, content, rank) in rows {
        let st: Option<(String,)> = sqlx::query_as(
            "SELECT source_type FROM chunk_map WHERE file = ?1 AND heading = ?2 AND content = ?3 LIMIT 1"
        )
        .bind(&file).bind(&heading).bind(&content)
        .fetch_optional(pool).await?;
        hits.push(SearchHit {
            file, heading,
            content: truncate(&content, 300),
            score: rank.abs(),
            source_type: st.map(|r| r.0).unwrap_or_else(|| "memory".into()),
        });
    }

    let filtered: Vec<SearchHit> = hits.into_iter().filter(|h| {
        if !q.filter_file.is_empty() && !h.file.starts_with(&q.filter_file) { return false; }
        if let Some(st) = &q.filter_source_type {
            if &h.source_type != st { return false; }
        }
        true
    }).take(top_k as usize).collect();

    Ok(filtered)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let mut end = max;
    while !s.is_char_boundary(end) { end -= 1; }
    s[..end].to_string()
}
```

Register in `lib.rs`:

```rust
pub mod search;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p dm-core --test search`
Expected: `5 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/search.rs crates/core/src/lib.rs crates/core/tests/search.rs
git commit -m "feat(core): BM25 search with file/source_type filters"
```

---

## Task 7: Temporal decay + search logging

**Files:**
- Modify: `/home/jeeves/directive-memory/crates/core/src/search.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/tests/search.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/Cargo.toml` (add `filetime` dev dep)

- [ ] **Step 1: Append failing tests to `tests/search.rs`**

```rust
#[tokio::test]
async fn newer_files_rank_higher_on_equal_match() {
    use dm_core::{db, indexer::{self, IndexRoot}, search::{self, SearchQuery}};
    use std::fs;
    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("old.md"), "# Old\nfencing discussion topic").unwrap();
    let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(180 * 86400);
    filetime::set_file_mtime(mem.join("old.md"),
        filetime::FileTime::from_system_time(old_time)).unwrap();
    fs::write(mem.join("new.md"), "# New\nfencing discussion topic").unwrap();

    let pool = db::open(&dir.path().join("t.db")).await.unwrap();
    indexer::reindex(&pool, &[IndexRoot { dir: mem, prefix: String::new() }]).await.unwrap();
    let hits = search::search(&pool, &SearchQuery {
        query: "fencing discussion".into(), top_k: 5, ..Default::default()
    }).await.unwrap();
    assert_eq!(hits[0].file, "new.md");
}

#[tokio::test]
async fn search_appends_row_to_log() {
    use dm_core::{db, indexer::{self, IndexRoot}, search::{self, SearchQuery}};
    use std::fs;
    use tempfile::tempdir;
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(mem.join("a.md"), "# A\nfencing").unwrap();
    let pool = db::open(&dir.path().join("l.db")).await.unwrap();
    indexer::reindex(&pool, &[IndexRoot { dir: mem, prefix: String::new() }]).await.unwrap();
    let before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_log")
        .fetch_one(&pool).await.unwrap();
    let _ = search::search(&pool, &SearchQuery {
        query: "fencing".into(), top_k: 3, ..Default::default()
    }).await.unwrap();
    let after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_log")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(after.0, before.0 + 1);
}
```

Add to `crates/core/Cargo.toml` dev-deps:

```toml
filetime = "0.2"
```

- [ ] **Step 2: Confirm failures**

Run: `cargo test -p dm-core --test search newer_files search_appends`
Expected: one ordering failure, one `search_log` count mismatch.

- [ ] **Step 3: Update `search.rs` — decay before truncation, then log**

Change the final `.take(top_k as usize).collect()` to retain the broader pool, then decay, then log, then truncate. Replace the final lines of `search` with:

```rust
    let filtered = apply_temporal_decay(pool, filtered).await?;
    log_search(pool, &sanitized, top_k, &filtered).await?;
    let filtered: Vec<SearchHit> = filtered.into_iter().take(top_k as usize).collect();
    Ok(filtered)
}
```

And append:

```rust
async fn apply_temporal_decay(pool: &SqlitePool, mut hits: Vec<SearchHit>) -> Result<Vec<SearchHit>> {
    use std::collections::{HashMap, HashSet};
    if hits.is_empty() { return Ok(hits); }
    let files: Vec<String> = hits.iter().map(|h| h.file.clone())
        .collect::<HashSet<_>>().into_iter().collect();
    let mut mtimes: HashMap<String, f64> = HashMap::new();
    for f in &files {
        if let Some((mt,)) = sqlx::query_as::<_, (f64,)>("SELECT mtime FROM files WHERE path = ?1")
            .bind(f).fetch_optional(pool).await? { mtimes.insert(f.clone(), mt); }
    }
    let now = chrono::Utc::now().timestamp() as f64;
    let half_life_days = 90.0_f64;
    for h in &mut hits {
        if let Some(mt) = mtimes.get(&h.file) {
            let days_old = ((now - mt) / 86_400.0).max(0.0);
            let decay = 0.5_f64.powf(days_old / half_life_days);
            h.score *= 0.5 + 0.5 * decay;
        }
    }
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(hits)
}

async fn log_search(pool: &SqlitePool, query: &str, top_k: i64, hits: &[SearchHit]) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();
    let top3: Vec<serde_json::Value> = hits.iter().take(3).map(|h| serde_json::json!({
        "file": h.file, "heading": h.heading, "score": h.score,
    })).collect();
    let top_results = serde_json::to_string(&top3).unwrap_or_else(|_| "[]".into());
    sqlx::query(
        "INSERT INTO search_log (ts, query, mode, top_k, result_count, top_results) \
         VALUES (?1, ?2, 'bm25', ?3, ?4, ?5)"
    )
    .bind(ts).bind(query).bind(top_k).bind(hits.len() as i64).bind(top_results)
    .execute(pool).await?;
    Ok(())
}
```

- [ ] **Step 4: Run all search tests**

Run: `cargo test -p dm-core --test search`
Expected: all 7 pass.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/search.rs crates/core/tests/search.rs crates/core/Cargo.toml
git commit -m "feat(core): temporal decay (90-day half-life) and search logging"
```

---

## Task 8: Write-back — write_file, append_file, add_fact

**Files:**
- Create: `/home/jeeves/directive-memory/crates/core/src/writeback.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/core/tests/writeback.rs`

Ports the Python path-validation regex and `add_fact` insertion logic. Writes disk only; reindex picks them up on the next call.

- [ ] **Step 1: Write failing tests**

```rust
// crates/core/tests/writeback.rs
use dm_core::writeback;
use std::fs;
use tempfile::tempdir;

#[test]
fn write_creates_file() {
    let dir = tempdir().unwrap();
    writeback::write_file(dir.path(), "notes.md", "# hi\nbody", false).unwrap();
    let body = fs::read_to_string(dir.path().join("notes.md")).unwrap();
    assert_eq!(body, "# hi\nbody");
}

#[test]
fn append_preserves_trailing_newline() {
    let dir = tempdir().unwrap();
    writeback::write_file(dir.path(), "x.md", "line one", false).unwrap();
    writeback::write_file(dir.path(), "x.md", "line two", true).unwrap();
    let body = fs::read_to_string(dir.path().join("x.md")).unwrap();
    assert!(body.starts_with("line one\n"));
    assert!(body.contains("line two"));
}

#[test]
fn rejects_path_traversal() {
    let dir = tempdir().unwrap();
    assert!(writeback::write_file(dir.path(), "../evil.md", "x", false).is_err());
    assert!(writeback::write_file(dir.path(), "a/../b.md", "x", false).is_err());
    assert!(writeback::write_file(dir.path(), "/etc/passwd.md", "x", false).is_err());
}

#[test]
fn rejects_non_markdown_extension() {
    let dir = tempdir().unwrap();
    assert!(writeback::write_file(dir.path(), "note.txt", "x", false).is_err());
}

#[test]
fn add_fact_creates_file_and_section_when_missing() {
    let dir = tempdir().unwrap();
    writeback::add_fact(dir.path(), "learnings.md", "## Patterns", "use sqlx").unwrap();
    let body = fs::read_to_string(dir.path().join("learnings.md")).unwrap();
    assert!(body.contains("# Learnings"));
    assert!(body.contains("## Patterns"));
    assert!(body.contains("- use sqlx"));
}

#[test]
fn add_fact_appends_under_existing_section() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("x.md"),
        "# X\n\n## Patterns\n- old one\n\n## Other\ndata\n").unwrap();
    writeback::add_fact(dir.path(), "x.md", "## Patterns", "new fact").unwrap();
    let body = fs::read_to_string(dir.path().join("x.md")).unwrap();
    let patterns_idx = body.find("## Patterns").unwrap();
    let other_idx    = body.find("## Other").unwrap();
    let slice = &body[patterns_idx..other_idx];
    assert!(slice.contains("- old one"));
    assert!(slice.contains("- new fact"));
}

#[test]
fn add_fact_appends_new_section_when_heading_absent() {
    let dir = tempdir().unwrap();
    fs::write(dir.path().join("x.md"), "# X\n\nbody\n").unwrap();
    writeback::add_fact(dir.path(), "x.md", "## New", "added").unwrap();
    let body = fs::read_to_string(dir.path().join("x.md")).unwrap();
    assert!(body.contains("## New"));
    assert!(body.contains("- added"));
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p dm-core --test writeback`
Expected: compile error.

- [ ] **Step 3: Implement `crates/core/src/writeback.rs`**

```rust
use crate::error::{CoreError, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

static SAFE_PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_\-/]+\.md$").unwrap());

fn validate(root: &Path, rel_path: &str) -> Result<PathBuf> {
    let rel = rel_path.trim();
    // Reject absolute paths outright — silently stripping a leading `/` would
    // be surprising (e.g. "/etc/passwd.md" turning into "etc/passwd.md") even
    // if the canonicalize check below caught it for existing files.
    if rel.starts_with('/') || rel.contains("..") || !SAFE_PATH_RE.is_match(rel) {
        return Err(CoreError::InvalidPath(rel_path.to_string()));
    }
    let full = root.join(rel);
    let canon_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    if let Ok(canon_full) = fs::canonicalize(&full) {
        if !canon_full.starts_with(&canon_root) {
            return Err(CoreError::InvalidPath(rel_path.to_string()));
        }
    }
    Ok(full)
}

pub fn write_file(root: &Path, rel_path: &str, content: &str, append: bool) -> Result<()> {
    let full = validate(root, rel_path)?;
    if let Some(parent) = full.parent() { fs::create_dir_all(parent)?; }
    if append && full.exists() {
        let mut existing = fs::read_to_string(&full)?;
        if !existing.ends_with('\n') { existing.push('\n'); }
        existing.push_str(content);
        fs::write(&full, existing)?;
    } else {
        fs::write(&full, content)?;
    }
    Ok(())
}

pub fn add_fact(root: &Path, rel_path: &str, section: &str, fact: &str) -> Result<()> {
    let full = validate(root, rel_path)?;
    let bullet = if fact.starts_with("- ") { fact.to_string() } else { format!("- {fact}") };

    if !full.exists() {
        if let Some(parent) = full.parent() { fs::create_dir_all(parent)?; }
        let title = rel_path.trim_end_matches(".md").replace('-', " ").replace('/', " — ");
        let title = titlecase(&title);
        let body = format!("# {title}\n\n{section}\n{bullet}\n");
        fs::write(&full, body)?;
        return Ok(());
    }

    let text = fs::read_to_string(&full)?;
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let section_trim = section.trim();
    let section_idx = lines.iter().position(|l| l.trim() == section_trim);

    match section_idx {
        Some(idx) => {
            let mut insert = idx + 1;
            while insert < lines.len() {
                let l = &lines[insert];
                if l.starts_with('#') && !l.starts_with("####") { break; }
                insert += 1;
            }
            while insert > idx + 1 && lines[insert - 1].trim().is_empty() {
                insert -= 1;
            }
            lines.insert(insert, bullet);
        }
        None => {
            if !lines.last().map(|l| l.is_empty()).unwrap_or(true) {
                lines.push(String::new());
            }
            lines.push(String::new());
            lines.push(section.to_string());
            lines.push(bullet);
        }
    }
    fs::write(&full, lines.join("\n"))?;
    Ok(())
}

fn titlecase(s: &str) -> String {
    s.split_whitespace().map(|w| {
        let mut chars = w.chars();
        match chars.next() {
            None => String::new(),
            Some(c) => c.to_uppercase().chain(chars.flat_map(|c| c.to_lowercase())).collect(),
        }
    }).collect::<Vec<_>>().join(" ")
}
```

Register in `lib.rs`:

```rust
pub mod writeback;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p dm-core --test writeback`
Expected: `7 passed`.

- [ ] **Step 5: Commit**

```bash
git add crates/core/src/writeback.rs crates/core/src/lib.rs crates/core/tests/writeback.rs
git commit -m "feat(core): safe markdown write-back with add_fact"
```

---

## Task 9: Stats

**Files:**
- Create: `/home/jeeves/directive-memory/crates/core/src/stats.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/src/lib.rs`

- [ ] **Step 1: Write module with inline tests**

```rust
// crates/core/src/stats.rs
use crate::error::Result;
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::BTreeMap;

#[derive(Debug, Serialize)]
pub struct Stats {
    pub chunks: i64,
    pub files: i64,
    pub source_types: BTreeMap<String, i64>,
    pub search_log_total: i64,
    pub search_log_last_7d: i64,
}

pub async fn gather(pool: &SqlitePool) -> Result<Stats> {
    let (chunks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM chunk_map").fetch_one(pool).await?;
    let (files,):  (i64,) = sqlx::query_as("SELECT COUNT(*) FROM files").fetch_one(pool).await?;
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT source_type, COUNT(*) FROM chunk_map GROUP BY source_type"
    ).fetch_all(pool).await?;
    let source_types: BTreeMap<String, i64> = rows.into_iter().collect();
    let (total,):  (i64,) = sqlx::query_as("SELECT COUNT(*) FROM search_log").fetch_one(pool).await?;
    let (recent,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM search_log WHERE ts >= datetime('now', '-7 days')"
    ).fetch_one(pool).await?;
    Ok(Stats { chunks, files, source_types, search_log_total: total, search_log_last_7d: recent })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, indexer::{self, IndexRoot}};
    use std::fs;
    use tempfile::tempdir;

    #[tokio::test]
    async fn reports_counts_and_source_types() {
        let dir = tempdir().unwrap();
        let mem = dir.path().join("memory");
        fs::create_dir_all(mem.join("projects")).unwrap();
        fs::write(mem.join("a.md"), "# A\ntext").unwrap();
        fs::write(mem.join("projects/b.md"), "# B\ntext").unwrap();
        let pool = db::open(&dir.path().join("s.db")).await.unwrap();
        indexer::reindex(&pool, &[IndexRoot { dir: mem, prefix: String::new() }]).await.unwrap();
        let s = gather(&pool).await.unwrap();
        assert_eq!(s.files, 2);
        assert!(s.chunks >= 2);
        assert_eq!(s.source_types.get("memory").copied().unwrap_or(0), 1);
        assert_eq!(s.source_types.get("project").copied().unwrap_or(0), 1);
    }
}
```

Register in `lib.rs`:

```rust
pub mod stats;
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p dm-core stats::tests`
Expected: `1 passed`.

- [ ] **Step 3: Commit**

```bash
git add crates/core/src/stats.rs crates/core/src/lib.rs
git commit -m "feat(core): stats aggregator"
```

---

## Task 10: Config loader + Core facade

**Files:**
- Create: `/home/jeeves/directive-memory/crates/core/src/config.rs`
- Create: `/home/jeeves/directive-memory/crates/core/src/core.rs`
- Modify: `/home/jeeves/directive-memory/crates/core/src/lib.rs`
- Create: `/home/jeeves/directive-memory/config.example.toml`

- [ ] **Step 1: Write `crates/core/src/config.rs`**

```rust
use crate::indexer::IndexRoot;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtraDir {
    pub dir: PathBuf,
    pub prefix: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub memory_dir: PathBuf,
    #[serde(default)]
    pub extra_dirs: Vec<ExtraDir>,
    pub db_path: PathBuf,
    pub port: u16,
    pub api_key: String,
    #[serde(default = "default_bind")]
    pub bind: String,
}

fn default_bind() -> String { "127.0.0.1".into() }

impl Default for Config {
    fn default() -> Self {
        Self {
            memory_dir: PathBuf::from("./memory"),
            extra_dirs: Vec::new(),
            db_path:    PathBuf::from("./data/memory.db"),
            port: 3001,
            api_key: String::new(),
            bind: default_bind(),
        }
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let mut builder = ::config::Config::builder()
            .set_default("memory_dir", "./memory")?
            .set_default("db_path", "./data/memory.db")?
            .set_default("port", 3001)?
            .set_default("bind", "127.0.0.1")?
            .set_default("api_key", "")?;
        if let Some(p) = path {
            builder = builder.add_source(::config::File::from(p).required(true));
        }
        builder = builder.add_source(
            ::config::Environment::with_prefix("DM").separator("__")
        );
        let cfg: Config = builder.build()?.try_deserialize()?;
        Ok(cfg)
    }

    pub fn index_roots(&self) -> Vec<IndexRoot> {
        let mut roots = vec![IndexRoot { dir: self.memory_dir.clone(), prefix: String::new() }];
        for e in &self.extra_dirs {
            roots.push(IndexRoot { dir: e.dir.clone(), prefix: e.prefix.clone() });
        }
        roots
    }
}
```

- [ ] **Step 2: Write `crates/core/src/core.rs`**

```rust
use crate::config::Config;
use crate::error::Result;
use crate::indexer::{self, IndexReport};
use crate::search::{self, SearchHit, SearchQuery};
use crate::stats::{self, Stats};
use crate::writeback;
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct Core {
    pub config: Arc<Config>,
    pub pool: SqlitePool,
}

impl Core {
    pub async fn open(config: Config) -> anyhow::Result<Self> {
        let pool = crate::db::open(&config.db_path).await?;
        Ok(Self { config: Arc::new(config), pool })
    }

    pub async fn reindex(&self) -> Result<IndexReport> {
        indexer::reindex(&self.pool, &self.config.index_roots()).await
    }

    pub async fn search(&self, q: &SearchQuery) -> Result<Vec<SearchHit>> {
        search::search(&self.pool, q).await
    }

    pub async fn stats(&self) -> Result<Stats> { stats::gather(&self.pool).await }

    pub fn write_file(&self, rel_path: &str, content: &str, append: bool) -> Result<()> {
        writeback::write_file(&self.config.memory_dir, rel_path, content, append)
    }
    pub fn add_fact(&self, rel_path: &str, section: &str, fact: &str) -> Result<()> {
        writeback::add_fact(&self.config.memory_dir, rel_path, section, fact)
    }

    pub async fn list_files(&self) -> Result<Vec<(String, f64)>> {
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT path, mtime FROM files ORDER BY path"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub fn read_file(&self, rel_path: &str) -> Result<String> {
        let root = &self.config.memory_dir;
        let rel = rel_path.trim().trim_start_matches('/');
        if rel.contains("..") {
            return Err(crate::error::CoreError::InvalidPath(rel_path.into()));
        }
        let full = root.join(rel);
        Ok(std::fs::read_to_string(full)?)
    }
}
```

- [ ] **Step 3: Wire into `lib.rs`**

```rust
//! Directive Memory core.
pub mod chunker;
pub mod config;
pub mod core;
pub mod db;
pub mod error;
pub mod indexer;
pub mod search;
pub mod source_type;
pub mod stats;
pub mod writeback;

pub use core::Core;
pub use error::{CoreError, Result};
```

- [ ] **Step 4: Write `config.example.toml`**

```toml
# Directive Memory config. All fields optional — defaults shown.

memory_dir = "./memory"
db_path    = "./data/memory.db"
port       = 3001
bind       = "127.0.0.1"

# Generate with: openssl rand -hex 32
api_key = ""

# Optional additional roots, e.g. an Obsidian vault.
# [[extra_dirs]]
# dir    = "/home/you/second_brain"
# prefix = "vault/"
```

- [ ] **Step 5: Build and commit**

Run: `cargo build -p dm-core`
Expected: clean build.

```bash
git add crates/core/src/config.rs crates/core/src/core.rs crates/core/src/lib.rs config.example.toml
git commit -m "feat(core): Core facade and config loader"
```

---

## Task 11: API skeleton — state, auth, error, health

**Files:**
- Create: `/home/jeeves/directive-memory/crates/api/src/{state,auth,error}.rs`
- Create: `/home/jeeves/directive-memory/crates/api/src/routes/mod.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/api/tests/health.rs`

- [ ] **Step 1: Write failing tests**

```rust
// crates/api/tests/health.rs
use axum::body::Body;
use axum::http::{Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::util::ServiceExt;

async fn setup() -> (tempfile::TempDir, axum::Router) {
    let dir = tempdir().unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = dir.path().join("memory");
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "test-key".into();
    std::fs::create_dir_all(&cfg.memory_dir).unwrap();
    let core = Core::open(cfg).await.unwrap();
    let router = build_router(core);
    (dir, router)
}

#[tokio::test]
async fn health_returns_ok_without_auth() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"ok");
}

#[tokio::test]
async fn api_routes_reject_missing_api_key() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(
        Request::builder().uri("/api/stats").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p dm-api --test health`
Expected: compile error.

- [ ] **Step 3: Implement `state.rs`**

```rust
use dm_core::Core;

#[derive(Clone)]
pub struct AppState {
    pub core: Core,
}
```

- [ ] **Step 4: Implement `error.rs`**

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self { status, message: message.into() }
    }
}

impl From<dm_core::CoreError> for ApiError {
    fn from(err: dm_core::CoreError) -> Self {
        use dm_core::CoreError::*;
        match err {
            InvalidPath(p) => ApiError::new(StatusCode::BAD_REQUEST, format!("invalid path: {p}")),
            Io(e) if e.kind() == std::io::ErrorKind::NotFound =>
                ApiError::new(StatusCode::NOT_FOUND, "file not found"),
            other => ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
```

- [ ] **Step 5: Implement `auth.rs`**

```rust
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use crate::state::AppState;

pub async fn require_api_key(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let expected = &state.core.config.api_key;
    if expected.is_empty() { return Err(StatusCode::UNAUTHORIZED); }
    let ok = req.headers().get("x-api-key")
        .and_then(|v| v.to_str().ok()).map(|v| v == expected).unwrap_or(false)
    || req.headers().get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|v| v == expected).unwrap_or(false);
    if !ok { return Err(StatusCode::UNAUTHORIZED); }
    Ok(next.run(req).await)
}
```

- [ ] **Step 6: Stub `routes/mod.rs`** (later tasks add files)

```rust
// Route modules registered in subsequent tasks.
```

- [ ] **Step 7: Implement `crates/api/src/lib.rs`**

```rust
//! Directive Memory HTTP API.

pub mod auth;
pub mod error;
pub mod routes;
pub mod state;

use axum::http::StatusCode;
use axum::{middleware, routing::get, Router};
use dm_core::Core;
use state::AppState;

pub fn build_router(core: Core) -> Router {
    let state = AppState { core };
    // A `.fallback` + `.layer` pair (rather than `.route_layer`) makes auth
    // fire for unknown `/api/*` paths too — preferable to 404 since it
    // avoids leaking which routes exist.
    let api = Router::new()
        .route("/_placeholder", get(|| async { "" }))
        .fallback(|| async { StatusCode::NOT_FOUND })
        .layer(middleware::from_fn_with_state(state.clone(), auth::require_api_key))
        .with_state(state.clone());

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api", api)
        .with_state(state)
}
```

(The `_placeholder` keeps the middleware attached even before real routes land; it's replaced in Task 12. The `.fallback` stays in place for the same reason — Task 12+ can keep or drop it depending on whether unknown `/api/*` paths should 401 or 404.)

- [ ] **Step 8: Run the tests**

Run: `cargo test -p dm-api --test health`
Expected: `2 passed`.

- [ ] **Step 9: Commit**

```bash
git add crates/api
git commit -m "feat(api): router skeleton with API-key auth and health check"
```

---

## Task 12: API — search route

**Files:**
- Create: `/home/jeeves/directive-memory/crates/api/src/routes/search.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/routes/mod.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/api/tests/search.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/api/tests/search.rs
use axum::body::Body;
use axum::http::{Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn search_returns_hits() {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    std::fs::write(mem.join("a.md"), "# A\nfencing quote").unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = mem.clone();
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "k".into();
    let core = Core::open(cfg).await.unwrap();
    core.reindex().await.unwrap();
    let app = build_router(core);

    let resp = app.oneshot(
        Request::builder()
            .uri("/api/search?q=fencing&top_k=3")
            .header("x-api-key", "k").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert!(v["hits"].as_array().unwrap().len() >= 1);
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p dm-api --test search`
Expected: 404 (route not mounted).

- [ ] **Step 3: Implement `routes/search.rs`**

```rust
use axum::extract::{Query, State};
use axum::Json;
use dm_core::search::{SearchHit, SearchQuery};
use serde::{Deserialize, Serialize};
use crate::{error::ApiError, state::AppState};

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub q: String,
    #[serde(default = "default_top_k")]
    pub top_k: i64,
    #[serde(default)]
    pub source_type: Option<String>,
    #[serde(default)]
    pub file_prefix: Option<String>,
}
fn default_top_k() -> i64 { 5 }

#[derive(Serialize)]
pub struct SearchResponse { pub query: String, pub hits: Vec<SearchHit> }

pub async fn handler(
    State(state): State<AppState>,
    Query(p): Query<SearchParams>,
) -> Result<Json<SearchResponse>, ApiError> {
    let hits = state.core.search(&SearchQuery {
        query: p.q.clone(),
        top_k: p.top_k,
        filter_file: p.file_prefix.unwrap_or_default(),
        filter_source_type: p.source_type,
    }).await?;
    Ok(Json(SearchResponse { query: p.q, hits }))
}
```

- [ ] **Step 4: Register in `routes/mod.rs` and mount in `lib.rs`**

```rust
// routes/mod.rs
pub mod search;
```

Replace the placeholder api block in `lib.rs` with:

```rust
let api = Router::new()
    .route("/search", get(routes::search::handler))
    .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_api_key))
    .with_state(state.clone());
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p dm-api --test search`
Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add crates/api
git commit -m "feat(api): GET /api/search"
```

---

## Task 13: API — files routes (list, read, write, append)

**Files:**
- Create: `/home/jeeves/directive-memory/crates/api/src/routes/files.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/routes/mod.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/api/tests/files.rs`

- [ ] **Step 1: Write failing tests**

```rust
// crates/api/tests/files.rs
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::util::ServiceExt;

async fn setup() -> (tempfile::TempDir, axum::Router) {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    std::fs::write(mem.join("a.md"), "# A\nhello").unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = mem.clone();
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "k".into();
    let core = Core::open(cfg).await.unwrap();
    core.reindex().await.unwrap();
    (dir, build_router(core))
}

fn req(method: Method, uri: &str, body: &str) -> Request<Body> {
    Request::builder().method(method).uri(uri)
        .header("x-api-key", "k")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string())).unwrap()
}

#[tokio::test]
async fn list_files_returns_indexed_files() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(req(Method::GET, "/api/files", "")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert!(v["files"].as_array().unwrap().iter().any(|f| f["path"] == "a.md"));
}

#[tokio::test]
async fn read_file_returns_content() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(req(Method::GET, "/api/files/a.md", "")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert!(v["content"].as_str().unwrap().contains("hello"));
}

#[tokio::test]
async fn write_then_read_roundtrip() {
    let (_d, app) = setup().await;
    let resp = app.clone().oneshot(req(
        Method::POST, "/api/files/new.md", r#"{"content":"# New\nbody"}"#
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app.oneshot(req(Method::GET, "/api/files/new.md", "")).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert!(v["content"].as_str().unwrap().contains("# New"));
}

#[tokio::test]
async fn append_adds_to_file() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(req(
        Method::PATCH, "/api/files/a.md", r#"{"content":"extra"}"#
    )).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn rejects_path_traversal() {
    let (_d, app) = setup().await;
    let resp = app.oneshot(req(
        Method::POST, "/api/files/..%2Fetc%2Fpasswd.md", r#"{"content":"x"}"#
    )).await.unwrap();
    assert!(resp.status().is_client_error());
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p dm-api --test files`
Expected: 404s.

- [ ] **Step 3: Implement `routes/files.rs`**

```rust
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use crate::{error::ApiError, state::AppState};

#[derive(Serialize)]
pub struct FileEntry { pub path: String, pub mtime: f64 }

#[derive(Serialize)]
pub struct ListResponse { pub files: Vec<FileEntry> }

pub async fn list(State(state): State<AppState>) -> Result<Json<ListResponse>, ApiError> {
    let rows = state.core.list_files().await?;
    Ok(Json(ListResponse {
        files: rows.into_iter().map(|(path, mtime)| FileEntry { path, mtime }).collect(),
    }))
}

#[derive(Serialize)]
pub struct FileContent { pub path: String, pub content: String }

pub async fn read(
    State(state): State<AppState>,
    Path(path): Path<String>,
) -> Result<Json<FileContent>, ApiError> {
    let content = state.core.read_file(&path)?;
    Ok(Json(FileContent { path, content }))
}

#[derive(Deserialize)]
pub struct WriteBody { pub content: String }

#[derive(Serialize)]
pub struct OkResponse { pub ok: bool, pub path: String }

pub async fn write(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(body): Json<WriteBody>,
) -> Result<Json<OkResponse>, ApiError> {
    state.core.write_file(&path, &body.content, false)?;
    Ok(Json(OkResponse { ok: true, path }))
}

pub async fn append(
    State(state): State<AppState>,
    Path(path): Path<String>,
    Json(body): Json<WriteBody>,
) -> Result<Json<OkResponse>, ApiError> {
    state.core.write_file(&path, &body.content, true)?;
    Ok(Json(OkResponse { ok: true, path }))
}
```

- [ ] **Step 4: Mount routes in `lib.rs`**

```rust
use axum::routing::get;

let api = Router::new()
    .route("/search", get(routes::search::handler))
    .route("/files", get(routes::files::list))
    .route("/files/*path",
        get(routes::files::read)
            .post(routes::files::write)
            .patch(routes::files::append))
    .route_layer(middleware::from_fn_with_state(state.clone(), auth::require_api_key))
    .with_state(state.clone());
```

`routes/mod.rs`:

```rust
pub mod files;
pub mod search;
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p dm-api --test files`
Expected: `5 passed`.

- [ ] **Step 6: Commit**

```bash
git add crates/api
git commit -m "feat(api): files list, read, write, append"
```

---

## Task 14: API — facts route

**Files:**
- Create: `/home/jeeves/directive-memory/crates/api/src/routes/facts.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/routes/mod.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/api/tests/facts.rs`

- [ ] **Step 1: Write the failing test**

```rust
// crates/api/tests/facts.rs
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn add_fact_writes_bullet() {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = mem.clone();
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "k".into();
    let core = Core::open(cfg).await.unwrap();
    let app = build_router(core);

    let resp = app.oneshot(
        Request::builder().method(Method::POST).uri("/api/facts")
            .header("x-api-key", "k").header("content-type", "application/json")
            .body(Body::from(
                r###"{"file":"learnings.md","section":"## Patterns","fact":"use sqlx"}"###
            )).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = std::fs::read_to_string(mem.join("learnings.md")).unwrap();
    assert!(body.contains("- use sqlx"));
}
```

- [ ] **Step 2: Implement `routes/facts.rs`**

```rust
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use crate::{error::ApiError, state::AppState};
use super::files::OkResponse;

#[derive(Deserialize)]
pub struct FactBody {
    pub file: String,
    pub section: String,
    pub fact: String,
}

pub async fn add(
    State(state): State<AppState>,
    Json(body): Json<FactBody>,
) -> Result<Json<OkResponse>, ApiError> {
    state.core.add_fact(&body.file, &body.section, &body.fact)?;
    Ok(Json(OkResponse { ok: true, path: body.file }))
}
```

- [ ] **Step 3: Register and mount**

`routes/mod.rs`:

```rust
pub mod facts;
```

`lib.rs` api router — add:

```rust
use axum::routing::post;
// ...
.route("/facts", post(routes::facts::add))
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p dm-api --test facts`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/api
git commit -m "feat(api): POST /api/facts"
```

---

## Task 15: API — stats + reindex routes

**Files:**
- Create: `/home/jeeves/directive-memory/crates/api/src/routes/stats.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/routes/mod.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/lib.rs`
- Test: `/home/jeeves/directive-memory/crates/api/tests/stats.rs`

- [ ] **Step 1: Write failing test**

```rust
// crates/api/tests/stats.rs
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use dm_api::build_router;
use dm_core::{config::Config, Core};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::util::ServiceExt;

#[tokio::test]
async fn stats_and_reindex() {
    let dir = tempdir().unwrap();
    let mem = dir.path().join("memory");
    std::fs::create_dir_all(&mem).unwrap();
    std::fs::write(mem.join("x.md"), "# X\ncontent").unwrap();
    let mut cfg = Config::default();
    cfg.memory_dir = mem.clone();
    cfg.db_path    = dir.path().join("db.sqlite");
    cfg.api_key    = "k".into();
    let core = Core::open(cfg).await.unwrap();
    let app = build_router(core);

    let resp = app.clone().oneshot(
        Request::builder().method(Method::POST).uri("/api/reindex")
            .header("x-api-key", "k").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = app.oneshot(
        Request::builder().uri("/api/stats")
            .header("x-api-key", "k").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(
        &resp.into_body().collect().await.unwrap().to_bytes()
    ).unwrap();
    assert_eq!(v["files"].as_i64().unwrap(), 1);
    assert!(v["chunks"].as_i64().unwrap() >= 1);
}
```

- [ ] **Step 2: Implement `routes/stats.rs`**

```rust
use axum::extract::State;
use axum::Json;
use serde::Serialize;
use crate::{error::ApiError, state::AppState};

pub async fn stats(State(state): State<AppState>)
    -> Result<Json<dm_core::stats::Stats>, ApiError>
{
    Ok(Json(state.core.stats().await?))
}

#[derive(Serialize)]
pub struct ReindexResponse { pub files_indexed: usize, pub files_pruned: usize }

pub async fn reindex(State(state): State<AppState>)
    -> Result<Json<ReindexResponse>, ApiError>
{
    let r = state.core.reindex().await?;
    Ok(Json(ReindexResponse { files_indexed: r.files_indexed, files_pruned: r.files_pruned }))
}
```

- [ ] **Step 3: Register and mount**

`routes/mod.rs`: add `pub mod stats;`

`lib.rs` api router:

```rust
.route("/stats", get(routes::stats::stats))
.route("/reindex", post(routes::stats::reindex))
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p dm-api --test stats`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/api
git commit -m "feat(api): stats and reindex endpoints"
```

---

## Task 16: API — serve the web UI as static assets

**Files:**
- Create: `/home/jeeves/directive-memory/crates/api/src/routes/static_ui.rs`
- Create: `/home/jeeves/directive-memory/web/.gitkeep` + placeholder `index.html`
- Modify: `/home/jeeves/directive-memory/crates/api/src/routes/mod.rs`
- Modify: `/home/jeeves/directive-memory/crates/api/src/lib.rs`

Bundles `web/` into the binary via `rust-embed`.

- [ ] **Step 1: Create `web/` with placeholder**

```bash
mkdir -p web
printf '<!doctype html><title>Directive Memory</title><h1>placeholder</h1>\n' > web/index.html
touch web/.gitkeep
```

- [ ] **Step 2: Implement `routes/static_ui.rs`**

```rust
use axum::body::Body;
use axum::extract::Path;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../web/"]
struct Assets;

pub async fn serve_root() -> Response { serve_path("index.html".into()).await }

pub async fn serve(Path(path): Path<String>) -> Response { serve_path(path).await }

async fn serve_path(path: String) -> Response {
    let asset = Assets::get(&path).or_else(|| Assets::get("index.html"));
    match asset {
        Some(file) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(file.data.into_owned()))
                .unwrap()
        }
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

pub fn router() -> axum::Router {
    axum::Router::new().route("/", get(serve_root)).route("/*path", get(serve))
}
```

- [ ] **Step 3: Mount in `lib.rs`**

```rust
Router::new()
    .route("/healthz", get(|| async { "ok" }))
    .nest("/api", api)
    .merge(routes::static_ui::router())
    .with_state(state)
```

Add `pub mod static_ui;` to `routes/mod.rs`.

- [ ] **Step 4: Build**

Run: `cargo build -p dm-api`
Expected: success.

- [ ] **Step 5: Commit**

```bash
git add crates/api web
git commit -m "feat(api): embed and serve static web UI"
```

---

## Task 17: MCP server skeleton

**Files:**
- Create: `/home/jeeves/directive-memory/crates/mcp/src/tools.rs`
- Modify: `/home/jeeves/directive-memory/crates/mcp/src/lib.rs`

- [ ] **Step 1: Implement `crates/mcp/src/lib.rs`**

```rust
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
```

- [ ] **Step 2: Stub `crates/mcp/src/tools.rs`**

```rust
use dm_core::Core;
use rmcp::{ServerHandler, model::{ServerInfo, Implementation}};

#[derive(Clone)]
pub struct MemoryServer { pub core: Core }

impl MemoryServer { pub fn new(core: Core) -> Self { Self { core } } }

impl ServerHandler for MemoryServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            server_info: Implementation {
                name: "directive-memory".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some(
                "Directive Memory — BM25 search over markdown files plus write-back.".into()
            ),
            ..Default::default()
        }
    }
}
```

> **API check before implementing tools (Tasks 18-19):** exact types in `rmcp` shift between 0.1 releases. Run `cargo doc -p rmcp --open` (or check docs.rs) to verify `ServerInfo` / `Implementation` / `ServiceExt` names match. If the compiled API has drifted materially, swap to Appendix A's hand-rolled JSON-RPC path — the tool names and argument shapes stay identical.

- [ ] **Step 3: Build**

Run: `cargo build -p dm-mcp`
Expected: success.

- [ ] **Step 4: Commit**

```bash
git add crates/mcp
git commit -m "feat(mcp): stdio server skeleton"
```

---

## Task 18: MCP — read tools (search, list_files, stats)

**Files:**
- Modify: `/home/jeeves/directive-memory/crates/mcp/src/tools.rs`

- [ ] **Step 1: Extend `tools.rs`**

```rust
use dm_core::search::SearchQuery;
use rmcp::{tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
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
fn default_top_k() -> i64 { 5 }

#[tool_router]
impl MemoryServer {
    #[tool(description = "BM25 search over memory files")]
    pub async fn search_memory(&self, args: SearchArgs) -> String {
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
        if hits.is_empty() { return format!("No results for '{}'.", args.query); }
        let mut out = format!("{} results for '{}':\n\n", hits.len(), args.query);
        for h in hits {
            out.push_str(&format!("[{}] {}\n  {}\n\n",
                h.file, h.heading, h.content.chars().take(200).collect::<String>()));
        }
        out
    }

    #[tool(description = "List indexed memory files")]
    pub async fn list_memory_files(&self) -> String {
        match self.core.list_files().await {
            Ok(files) if files.is_empty() => "No memory files indexed.".into(),
            Ok(files) => files.into_iter()
                .map(|(p, _)| format!("- {p}")).collect::<Vec<_>>().join("\n"),
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
                for (k, v) in s.source_types { out.push_str(&format!("  {k}: {v}\n")); }
                out
            }
            Err(e) => format!("error: {e}"),
        }
    }
}
```

> If `rmcp` requires all tools in one `#[tool_router]` impl block, merge Task 19's definitions into this one when landing Task 19.

- [ ] **Step 2: Build**

Run: `cargo build -p dm-mcp`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/mcp
git commit -m "feat(mcp): search_memory, list_memory_files, get_memory_stats"
```

---

## Task 19: MCP — write tools (write_memory, add_fact)

**Files:**
- Modify: `/home/jeeves/directive-memory/crates/mcp/src/tools.rs`

- [ ] **Step 1: Extend `tools.rs`**

```rust
#[derive(Debug, Deserialize, JsonSchema)]
pub struct WriteMemoryArgs {
    /// Relative path within memory_dir (e.g., "projects/sift.md")
    pub file_path: String,
    /// Markdown content to write
    pub content: String,
    /// If true, append. If false (default), overwrite.
    #[serde(default)]
    pub append: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddFactArgs {
    pub file_path: String,
    /// Section heading (e.g., "## Patterns")
    pub section: String,
    /// Fact to add (formatted as "- <fact>")
    pub fact: String,
}

#[tool_router]
impl MemoryServer {
    #[tool(description = "Write or append to a memory file")]
    pub async fn write_memory(&self, args: WriteMemoryArgs) -> String {
        match self.core.write_file(&args.file_path, &args.content, args.append) {
            Ok(()) => format!(
                "{} {}",
                if args.append { "Appended to" } else { "Wrote" },
                args.file_path
            ),
            Err(e) => format!("error: {e}"),
        }
    }

    #[tool(description = "Add a fact as a bullet under a section in a memory file")]
    pub async fn add_fact(&self, args: AddFactArgs) -> String {
        match self.core.add_fact(&args.file_path, &args.section, &args.fact) {
            Ok(()) => format!("Added fact to {} under '{}'", args.file_path, args.section),
            Err(e) => format!("error: {e}"),
        }
    }
}
```

- [ ] **Step 2: Build**

Run: `cargo build -p dm-mcp`
Expected: success.

- [ ] **Step 3: Commit**

```bash
git add crates/mcp
git commit -m "feat(mcp): write_memory and add_fact"
```

---

## Task 20: Web UI — DOM-safe search, results, file browser, viewer

**Files:**
- Modify: `/home/jeeves/directive-memory/web/index.html`
- Create: `/home/jeeves/directive-memory/web/styles.css`
- Create: `/home/jeeves/directive-memory/web/app.js`

Minimal vanilla-JS UI with **no `innerHTML` for data paths**. All user/server data is inserted via `textContent` or `createElement`. Markdown is rendered through `marked` and sanitized via `DOMPurify` before the *only* `innerHTML` assignment in the code — the rendered markdown article. Both libraries load from CDN.

- [ ] **Step 1: Write `web/index.html`**

```html
<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Directive Memory</title>
  <link rel="stylesheet" href="/styles.css">
  <script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
  <script src="https://cdn.jsdelivr.net/npm/dompurify/dist/purify.min.js"></script>
</head>
<body>
  <header>
    <h1>Directive Memory</h1>
    <form id="search-form">
      <input id="q" type="search" placeholder="Search your knowledge…" autofocus>
      <input id="apikey" type="password" placeholder="API key" aria-label="API key">
      <button type="submit">Search</button>
    </form>
  </header>
  <main>
    <aside id="sidebar">
      <h2>Files</h2>
      <ul id="file-list"></ul>
    </aside>
    <section id="results">
      <h2>Results</h2>
      <ol id="hits"></ol>
    </section>
    <section id="viewer">
      <h2 id="viewer-title">Select a file</h2>
      <article id="viewer-body"></article>
    </section>
  </main>
  <script src="/app.js"></script>
</body>
</html>
```

- [ ] **Step 2: Write `web/styles.css`**

```css
* { box-sizing: border-box; }
body { margin: 0; font: 14px/1.5 -apple-system, system-ui, sans-serif; color: #111; background: #fafafa; }
header { padding: 12px 20px; background: #fff; border-bottom: 1px solid #e0e0e0; display: flex; align-items: center; gap: 16px; }
header h1 { font-size: 16px; margin: 0; }
form { display: flex; gap: 8px; flex: 1; }
input[type="search"] { flex: 1; padding: 8px 10px; border: 1px solid #ccc; border-radius: 4px; }
input[type="password"] { width: 200px; padding: 8px; border: 1px solid #ccc; border-radius: 4px; }
button { padding: 8px 14px; background: #111; color: #fff; border: 0; border-radius: 4px; cursor: pointer; }
main { display: grid; grid-template-columns: 240px 1fr 1fr; gap: 16px; padding: 16px; }
aside, section { background: #fff; border: 1px solid #e0e0e0; border-radius: 6px; padding: 12px 14px; max-height: calc(100vh - 100px); overflow: auto; }
#file-list { list-style: none; padding: 0; margin: 0; }
#file-list li { padding: 4px 6px; border-radius: 4px; cursor: pointer; color: #333; }
#file-list li:hover { background: #f0f0f0; }
#hits { list-style: decimal; padding-left: 20px; }
#hits li { margin-bottom: 12px; }
#hits .file { font-weight: 600; color: #0366d6; cursor: pointer; }
#hits .heading { color: #555; font-size: 13px; }
#hits .snippet { color: #444; margin-top: 4px; white-space: pre-wrap; }
#viewer-body { font-size: 14px; }
#viewer-body pre { background: #f6f8fa; padding: 10px; border-radius: 4px; overflow: auto; }
.error { color: #c00; }
```

- [ ] **Step 3: Write `web/app.js` — DOM-safe throughout**

```js
const $ = (id) => document.getElementById(id);
const apiKeyKey = "dm-api-key";
$("apikey").value = localStorage.getItem(apiKeyKey) || "";
$("apikey").addEventListener("change", (e) => localStorage.setItem(apiKeyKey, e.target.value));

async function api(path, opts = {}) {
  const key = $("apikey").value;
  const res = await fetch(path, {
    ...opts,
    headers: { "x-api-key": key, "content-type": "application/json", ...(opts.headers || {}) },
  });
  if (!res.ok) throw new Error(`${res.status} ${await res.text()}`);
  return res.json();
}

function clear(el) { while (el.firstChild) el.removeChild(el.firstChild); }

function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === "class") node.className = v;
    else if (k === "dataset") Object.assign(node.dataset, v);
    else node.setAttribute(k, v);
  }
  for (const c of children) {
    if (typeof c === "string") node.appendChild(document.createTextNode(c));
    else if (c) node.appendChild(c);
  }
  return node;
}

function showError(container, message) {
  clear(container);
  container.appendChild(el("li", { class: "error" }, [message]));
}

async function loadFiles() {
  const list = $("file-list");
  try {
    const { files } = await api("/api/files");
    clear(list);
    for (const f of files) {
      const li = el("li", { dataset: { path: f.path } }, [f.path]);
      li.addEventListener("click", () => openFile(f.path));
      list.appendChild(li);
    }
  } catch (e) {
    showError(list, e.message);
  }
}

async function openFile(path) {
  try {
    const { content } = await api(`/api/files/${encodeURIComponent(path)}`);
    $("viewer-title").textContent = path;
    const html = DOMPurify.sanitize(marked.parse(content));
    $("viewer-body").innerHTML = html;
  } catch (e) {
    $("viewer-title").textContent = "Error";
    const body = $("viewer-body");
    clear(body);
    body.appendChild(el("p", { class: "error" }, [e.message]));
  }
}

async function search(q) {
  const hitsEl = $("hits");
  try {
    const { hits } = await api(`/api/search?q=${encodeURIComponent(q)}&top_k=10`);
    clear(hitsEl);
    for (const h of hits) {
      const fileLink = el("div", { class: "file", dataset: { path: h.file } }, [h.file]);
      fileLink.addEventListener("click", () => openFile(h.file));
      const li = el("li", {}, [
        fileLink,
        el("div", { class: "heading" }, [h.heading]),
        el("div", { class: "snippet" }, [h.content]),
      ]);
      hitsEl.appendChild(li);
    }
  } catch (e) {
    showError(hitsEl, e.message);
  }
}

$("search-form").addEventListener("submit", (e) => {
  e.preventDefault();
  search($("q").value.trim());
});
loadFiles();
```

> Why this shape: the only `innerHTML` write is the sanitized markdown render, so untrusted file/heading/snippet strings never reach the HTML parser. `DOMPurify` strips `<script>` and event handlers even if a markdown file contains raw HTML.

- [ ] **Step 4: Rebuild the API crate so embedded assets update**

Run: `cargo build -p dm-api`
Expected: success (rust-embed reads the new files at compile time).

- [ ] **Step 5: Commit**

```bash
git add web
git commit -m "feat(web): DOM-safe search, results, file browser, markdown viewer"
```

---

## Task 21: Binary — `clap` subcommands (serve, mcp, reindex, search)

**Files:**
- Modify: `/home/jeeves/directive-memory/src/main.rs`

- [ ] **Step 1: Replace `src/main.rs`**

```rust
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
    tracing_subscriber::fmt()
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
        Command::Mcp => dm_mcp::run_stdio(core).await?,
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
    let report = core.reindex().await?;
    tracing::info!(indexed = report.files_indexed, pruned = report.files_pruned, "startup reindex");
    let addr: std::net::SocketAddr = format!("{}:{}", core.config.bind, core.config.port).parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");
    let router = dm_api::build_router(core);
    axum::serve(listener, router).await?;
    Ok(())
}
```

- [ ] **Step 2: Build release binary**

Run: `cargo build --release`
Expected: `target/release/directive-memory` exists.

- [ ] **Step 3: Smoke-test**

```bash
rm -rf /tmp/dm-smoke && mkdir -p /tmp/dm-smoke/memory
printf '# Hello\nfencing discussion\n' > /tmp/dm-smoke/memory/a.md
cat > /tmp/dm-smoke/config.toml <<'EOF'
memory_dir = "/tmp/dm-smoke/memory"
db_path    = "/tmp/dm-smoke/data/db.sqlite"
port       = 3099
api_key    = "smoke-key"
EOF
./target/release/directive-memory --config /tmp/dm-smoke/config.toml reindex
./target/release/directive-memory --config /tmp/dm-smoke/config.toml search "fencing"
```

Expected: reindex reports 1 file, search returns a JSON array containing the hit.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: clap CLI with serve/mcp/reindex/search"
```

---

## Task 22: Binary size check + release polish

**Files:**
- (maybe) Modify: dependency feature flags

- [ ] **Step 1: Measure**

Run: `ls -l target/release/directive-memory`
Goal: ≤10 MB after strip.

- [ ] **Step 2: If oversize, trim**

```bash
cargo tree --workspace --edges=normal | head -100
```

Typical trims:
- Confirm `sqlx` has `default-features = false` (already set)
- Narrow `tower-http` features if not used
- Confirm dev-only deps don't leak into the binary

Only re-measure after trimming. Record the final size in the PR description.

- [ ] **Step 3: Run the full test suite in release mode**

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 4: Commit if anything changed**

```bash
git add Cargo.toml crates
git commit -m "chore: tighten release build features"
```

(Skip if unchanged.)

---

## Task 23: README, docs, final polish

**Files:**
- Modify: `/home/jeeves/directive-memory/README.md`

- [ ] **Step 1: Write full `README.md`**

````markdown
# Directive Memory

An open-source, MCP-native personal knowledge base. Point it at a directory of
markdown files and query your knowledge from Claude Desktop, the REST API, the
CLI, or the built-in web UI. Single Rust binary, SQLite under the hood, no
external services required.

Part of [Directive Systems](https://directive.systems).

## Features

- **MCP server** — plug your notes into any MCP-capable AI client
- **REST API** — use from mobile apps, scripts, or custom frontends
- **Web UI** — search and browse served from the same binary
- **Markdown-native** — your files stay as markdown on disk
- **BM25 search with temporal decay**; vector + cross-encoder ranking on the roadmap
- **Write-back** — AI clients can add facts, not just read them
- **Self-hosted** — nothing leaves your machine

## Quickstart

```bash
cargo install --path .                        # or download a release binary
mkdir -p ~/notes
printf '# Hello\nMy first note.\n' > ~/notes/hello.md

cat > ~/dm.toml <<EOF
memory_dir = "~/notes"
db_path    = "~/.local/share/directive-memory/db.sqlite"
port       = 3001
api_key    = "$(openssl rand -hex 32)"
EOF

directive-memory --config ~/dm.toml serve
```

Open http://127.0.0.1:3001 and paste the `api_key` from your config.

## REST API

All `/api/*` routes require the `x-api-key` header.

| Method | Path                  | Body / Query                                          |
|--------|-----------------------|-------------------------------------------------------|
| GET    | `/api/search`         | `?q=&top_k=&source_type=&file_prefix=`                |
| GET    | `/api/files`          | —                                                     |
| GET    | `/api/files/{path}`   | —                                                     |
| POST   | `/api/files/{path}`   | `{"content":"..."}`                                   |
| PATCH  | `/api/files/{path}`   | `{"content":"..."}` (appended)                        |
| POST   | `/api/facts`          | `{"file":"...","section":"## ...","fact":"..."}`      |
| GET    | `/api/stats`          | —                                                     |
| POST   | `/api/reindex`        | —                                                     |

## MCP setup (Claude Desktop)

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "directive-memory": {
      "command": "/absolute/path/to/directive-memory",
      "args": ["--config", "/absolute/path/to/dm.toml", "mcp"]
    }
  }
}
```

Available tools: `search_memory`, `list_memory_files`, `get_memory_stats`,
`write_memory`, `add_fact`.

## Config

See `config.example.toml`. Every key can also be set via `DM_*` env vars
(`DM_PORT=4000`, `DM_API_KEY=xxx`, etc.).

Extra roots — including Obsidian vaults — are indexed under a virtual prefix:

```toml
[[extra_dirs]]
dir    = "/home/you/second_brain"
prefix = "vault/"
```

## CLI

```
directive-memory serve       # REST API + web UI
directive-memory mcp         # MCP over stdio
directive-memory reindex     # one-shot full reindex
directive-memory search "q"  # JSON-formatted search
```

## License

AGPLv3. Hosted commercial version available at directive.systems — see LICENSE.
````

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: README with quickstart, REST, MCP setup"
```

---

## Appendix A — Hand-rolled MCP fallback

Swap for Tasks 17-19 if `rmcp`'s API has drifted. Tool names and argument
shapes stay identical.

```rust
// crates/mcp/src/lib.rs
use anyhow::Result;
use dm_core::Core;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn run_stdio(core: Core) -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();

    while let Some(line) = reader.next_line().await? {
        let Ok(req): serde_json::Result<Value> = serde_json::from_str(&line) else { continue };
        let method = req["method"].as_str().unwrap_or("");
        let id = req["id"].clone();
        let params = req["params"].clone();

        let result = match method {
            "initialize" => json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": {"name": "directive-memory", "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"tools": {}}
            }),
            "tools/list" => json!({ "tools": tool_definitions() }),
            "tools/call" => dispatch(&core, &params).await.unwrap_or_else(|e|
                json!({"isError": true, "content": [{"type":"text","text": e.to_string()}]})
            ),
            _ => json!({"error": {"code": -32601, "message": "method not found"}}),
        };
        let resp = json!({ "jsonrpc": "2.0", "id": id, "result": result });
        stdout.write_all(format!("{resp}\n").as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

fn tool_definitions() -> Value {
    json!([
        {"name": "search_memory", "description": "BM25 search over memory files",
         "inputSchema": {"type":"object","properties":{
            "query":{"type":"string"},
            "top_k":{"type":"integer","default":5},
            "filter_file":{"type":"string","default":""},
            "filter_source_type":{"type":"string","default":""}},
          "required":["query"]}},
        // …repeat for list_memory_files, get_memory_stats, write_memory, add_fact
    ])
}

async fn dispatch(core: &Core, params: &Value) -> anyhow::Result<Value> {
    let name = params["name"].as_str().ok_or_else(|| anyhow::anyhow!("missing name"))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let text = match name {
        "search_memory"     => { /* build SearchQuery from args, call core.search, format */ String::new() }
        "list_memory_files" => { /* call core.list_files, format */ String::new() }
        "get_memory_stats"  => { /* call core.stats, format */ String::new() }
        "write_memory"      => { /* call core.write_file, return confirmation */ String::new() }
        "add_fact"          => { /* call core.add_fact, return confirmation */ String::new() }
        _ => anyhow::bail!("unknown tool: {name}"),
    };
    Ok(json!({ "content": [{"type": "text", "text": text}] }))
}
```

Flesh out the `{ … }` blocks from the Task 18/19 logic if this fallback is
used. The `rmcp` path is preferred.

---

## Self-Review

**Spec coverage check:**

| Spec item | Task |
|---|---|
| Workspace: `core`, `api`, `mcp` crates | Task 1 |
| Deps: axum, sqlx (SQLite), tokio, serde, thiserror, anyhow, tower-http, chrono | Task 1 |
| Migrations: files, chunks FTS5, chunk_map, search_log, meta | Task 2 |
| File indexer (mtime diffing, pruning) | Task 5 |
| Chunker (heading split, 800-char paragraph subdivide) | Task 3 |
| Source type classifier | Task 4 |
| BM25 search with filters | Task 6 |
| Temporal decay (90-day half-life) | Task 7 |
| Search log | Task 7 |
| Write-back (write_file / append / add_fact + path safety) | Task 8 |
| Stats | Task 9 |
| Config (TOML + env, memory_dir / extra_dirs / db_path / port / api_key) | Task 10 |
| REST endpoints (search, files, facts, stats, reindex) | Tasks 12-15 |
| API-key auth | Task 11 |
| Static web UI served from the same binary | Tasks 16, 20 |
| MCP stdio server | Task 17 |
| MCP tools (search, list_files, stats, write, add_fact) | Tasks 18-19 |
| Binary subcommands (serve, mcp, reindex, search) | Task 21 |
| Release binary ≤10 MB | Task 22 |
| README + MCP setup | Task 23 |
| UUID dep listed in spec | Deliberately omitted — nothing in v1 uses it |

**Intentional omissions (per the non-goals list):**
- Vector embeddings / cross-encoder / MMR — `meta` table reserved, no code
- PostgreSQL — `sqlx` can abstract later; v1 uses `SqlitePool` directly
- Real-time file watching — startup reindex + `/api/reindex` endpoint
- Conversation source type — phase 2

**Type consistency pass:** `SearchHit` fields align across core/API/MCP. `SourceType::as_str()` strings match filter params and stats grouping keys. `IndexRoot { dir, prefix }` consistent in tests and implementation. `OkResponse` reused between `files.rs` and `facts.rs`.

**No placeholders detected.**

---

## Risks / open questions

1. **`rmcp` API drift** — mitigated by Appendix A. Verify types against docs.rs before Task 17.
2. **Migrations path resolution** — the symlink approach is POSIX-only. Windows: switch macro to `sqlx::migrate!("../../migrations")`.
3. **Binary size target (10 MB)** — realistic with LTO+strip but not guaranteed. Task 22 measures; if it lands at 12-14 MB, note in PR and defer slim work to phase 2.
4. **axum 0.7 path capture (`*path`)** — URL-encoded slashes decode into the capture. The traversal test in Task 13 exercises this path; verify behavior matches expectations on current axum minor version.
5. **DOMPurify via CDN** — the web UI fetches DOMPurify/marked from jsdelivr at runtime. If offline-first is required, vendor both JS files into `web/` and reference locally.
