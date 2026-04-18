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

All `/api/*` routes require the `x-api-key` header (or `Authorization: Bearer <key>`).

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

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS)
or the equivalent on your OS:

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

See `config.example.toml` for all options. Every key can also be set via
`DM_*` environment variables (`DM_PORT=4000`, `DM_API_KEY=xxx`, etc.).

Extra roots — including Obsidian vaults — are indexed under a virtual prefix:

```toml
[[extra_dirs]]
dir    = "/home/you/second_brain"
prefix = "vault/"
```

Files under an extra root get their path prefixed (e.g.
`vault/daily/2026-04-18.md`) and classified with the matching `source_type`,
so search filters (`filter_source_type=vault`) work as expected.

## CLI

```
directive-memory serve       # REST API + web UI
directive-memory mcp         # MCP over stdio
directive-memory reindex     # one-shot full reindex
directive-memory search "q"  # JSON-formatted search
```

## Architecture

- `crates/core` — domain, SQLite schema, indexer, search, write-back
- `crates/api` — axum REST + static web UI (via `rust-embed`)
- `crates/mcp` — stdio MCP server using the `rmcp` SDK
- `src/main.rs` — clap CLI that composes the three crates

## License

AGPLv3. See `LICENSE` for the full text. A hosted commercial version is
planned at directive.systems.
