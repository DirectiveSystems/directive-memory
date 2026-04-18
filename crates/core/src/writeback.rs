use crate::error::{CoreError, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

static SAFE_PATH_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[a-zA-Z0-9_\-/]+\.md$").unwrap());

fn validate(root: &Path, rel_path: &str) -> Result<PathBuf> {
    let rel = rel_path.trim();
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
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent)?;
    }
    if append && full.exists() {
        let mut existing = fs::read_to_string(&full)?;
        if !existing.ends_with('\n') {
            existing.push('\n');
        }
        existing.push_str(content);
        fs::write(&full, existing)?;
    } else {
        fs::write(&full, content)?;
    }
    Ok(())
}

pub fn add_fact(root: &Path, rel_path: &str, section: &str, fact: &str) -> Result<()> {
    let full = validate(root, rel_path)?;
    let bullet = if fact.starts_with("- ") {
        fact.to_string()
    } else {
        format!("- {fact}")
    };

    if !full.exists() {
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
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
            // Find end of this section: next h1/h2/h3 or EOF.
            let mut insert = idx + 1;
            while insert < lines.len() {
                let l = &lines[insert];
                if l.starts_with('#') && !l.starts_with("####") {
                    break;
                }
                insert += 1;
            }
            // Back up past trailing blank lines so the bullet sits with the section.
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
    s.split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c
                    .to_uppercase()
                    .chain(chars.flat_map(|c| c.to_lowercase()))
                    .collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
