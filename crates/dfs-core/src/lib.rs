pub mod agent;
pub mod db;
pub mod ledger;
pub mod llm;
pub mod models;
pub mod scanner;
pub mod watcher;

use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;

pub struct DollarFs {
    pub db_path: PathBuf,
    pub config_dir: PathBuf,
}

impl DollarFs {
    pub fn init(config_dir: Option<PathBuf>) -> Result<Self> {
        let config_dir = config_dir.unwrap_or_else(|| {
            dirs::home_dir()
                .expect("home dir")
                .join(".local_file_value")
        });
        std::fs::create_dir_all(&config_dir)?;

        let db_path = config_dir.join("lfv.db");
        let mut conn = db::init_db(&db_path)?;
        db::ensure_settings(&mut conn)?;

        Ok(Self {
            db_path,
            config_dir,
        })
    }

    pub fn open_db(&self) -> Result<Connection> {
        Ok(rusqlite::Connection::open(&self.db_path)?)
    }
}

pub fn default_watched_dirs() -> Vec<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    ["Desktop", "Documents", "Downloads", "Developer", "Projects", "Code"]
        .iter()
        .map(|d| home.join(d))
        .collect()
}
