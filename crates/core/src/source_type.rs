//! Source type classifier. Determines which corpus a file belongs to based
//! on the leading component of its repo-relative path.

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
