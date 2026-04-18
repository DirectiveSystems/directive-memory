//! Markdown chunker.
//!
//! Splits a document by h1-h4 headings into `Chunk` records; sections longer
//! than `MAX_CHUNK_CHARS` are further subdivided by paragraph grouping.
//! Pure functions, no I/O. Semantics mirror the Python reference at
//! `claude-ops/scripts/jeeves_lib/memory_search.py` (`_parse_chunks` /
//! `_split_by_paragraphs`).
//!
//! Known limitation: we don't track markdown code fences, so `##` lines
//! inside a ```` ``` ```` block are mis-classified as headings. Acceptable
//! for v1 — matches Python behaviour — but worth revisiting if docs with
//! markdown-inside-fences become common in the indexed corpus.

use once_cell::sync::Lazy;
use regex::Regex;

/// Soft cap on chunk size, measured in **bytes** (not characters).
///
/// For ASCII-heavy markdown this is ~200 tokens. For CJK or emoji text a
/// single character may be 3-4 bytes, so chunks end up smaller than the
/// name suggests. We keep the name `CHARS` to mirror the Python reference,
/// which uses `len(str)` (code-point count) — the divergence is known and
/// accepted for v1 since BM25 tokenization is byte-oriented anyway.
pub const MAX_CHUNK_CHARS: usize = 800;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub heading: String,
    pub content: String,
}

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

    #[test]
    fn hard_split_fires_on_single_huge_paragraph() {
        // One paragraph with many short lines and no blank lines.
        // Total bytes must exceed MAX_CHUNK_CHARS * 2 so the hard-split
        // `while buf_chars > max_chars * 2` loop fires.
        let huge_paragraph = (0..500)
            .map(|i| format!("line {i} with some content here"))
            .collect::<Vec<_>>()
            .join("\n");
        let md = format!("# Giant\n{huge_paragraph}\n");
        let chunks = parse_chunks(&md);
        assert!(chunks.len() >= 2, "hard-split path should produce multiple chunks");
        for c in &chunks {
            assert!(
                c.content.len() <= MAX_CHUNK_CHARS * 2,
                "each chunk content must stay within the 2x bound; got {}",
                c.content.len()
            );
        }
    }

    #[test]
    fn many_small_paragraphs_cluster_into_one_chunk() {
        // 10 tiny paragraphs (~20 bytes each) summing to well under MAX_CHUNK_CHARS.
        // Should come back as exactly one chunk (section-level, not subdivided).
        let paragraphs: Vec<String> = (0..10).map(|i| format!("short para {i}.")).collect();
        let md = format!("# Cluster\n{}\n", paragraphs.join("\n\n"));
        let chunks = parse_chunks(&md);
        assert_eq!(chunks.len(), 1, "small paragraphs should not subdivide");
        assert_eq!(chunks[0].heading, "Cluster");
        for p in &paragraphs {
            assert!(chunks[0].content.contains(p), "missing paragraph: {p}");
        }
    }
}
