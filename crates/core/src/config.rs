//! Config loader. TOML file + `DM_*` environment variables, with sensible defaults.

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
