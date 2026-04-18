use once_cell::sync::Lazy;
use regex::Regex;

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
}
