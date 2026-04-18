-- Tracks indexed markdown files for incremental reindex
CREATE TABLE files (
    path  TEXT PRIMARY KEY,
    mtime REAL NOT NULL
);
CREATE INDEX idx_files_path ON files(path);

-- Full-text search index (BM25 ranking built in)
CREATE VIRTUAL TABLE chunks USING fts5(
    file, heading, content
);

-- Canonical chunk storage with metadata
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
CREATE INDEX idx_chunk_map_file ON chunk_map(file);
CREATE INDEX idx_chunk_map_source_type ON chunk_map(source_type);

-- Search telemetry
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

-- Model metadata (reserved for phase-2 vector drift detection)
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
