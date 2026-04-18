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
    }).collect();

    let filtered = apply_temporal_decay(pool, filtered).await?;
    log_search(pool, &sanitized, top_k, &filtered).await?;
    let filtered: Vec<SearchHit> = filtered.into_iter().take(top_k as usize).collect();

    Ok(filtered)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    let mut end = max;
    while !s.is_char_boundary(end) { end -= 1; }
    s[..end].to_string()
}

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
