use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Append-only hash-chained JSONL ledger.
/// Each entry references the previous hash, forming an immutable chain.
/// Compatible with the Python MembraLedger in safe_git_watcher/ledger.py.
#[derive(Debug, Clone)]
pub struct MembraLedger {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerBlock {
    pub index: u64,
    pub timestamp: String,
    pub previous_hash: String,
    pub data: serde_json::Value,
    pub hash: String,
}

impl MembraLedger {
    pub fn new(path: &Path) -> Self {
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")));
        if !path.exists() {
            let _ = std::fs::write(path, "");
        }
        Self {
            path: path.to_path_buf(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    pub fn from_config_dir(config_dir: &Path) -> Self {
        Self::new(&config_dir.join("membra_ledger.jsonl"))
    }

    fn compute_hash(block: &LedgerBlock) -> String {
        let payload = serde_json::to_string(block).unwrap_or_default();
        let digest = blake3::hash(payload.as_bytes());
        digest.to_hex().to_string()
    }

    fn last_block(&self) -> Option<LedgerBlock> {
        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return None,
        };
        let reader = std::io::BufReader::new(file);
        let mut last = None;
        for line in reader.lines().flatten() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(block) = serde_json::from_str::<LedgerBlock>(line) {
                last = Some(block);
            }
        }
        last
    }

    pub fn append(&self, data: serde_json::Value) -> Result<LedgerBlock> {
        let _guard = self.lock.lock().unwrap();
        let last = self.last_block();
        let index = last.as_ref().map(|b| b.index + 1).unwrap_or(0);
        let previous_hash = last.map(|b| b.hash).unwrap_or_else(|| "0".repeat(64));
        let timestamp = chrono::Utc::now().to_rfc3339();

        let mut block = LedgerBlock {
            index,
            timestamp,
            previous_hash,
            data,
            hash: String::new(),
        };
        block.hash = Self::compute_hash(&block);

        let line = serde_json::to_string(&block)?;
        let mut file = OpenOptions::new().append(true).create(true).open(&self.path)?;
        writeln!(file, "{}", line)?;
        Ok(block)
    }

    pub fn get_chain(&self, limit: Option<usize>, offset: usize) -> Vec<LedgerBlock> {
        let file = match std::fs::File::open(&self.path) {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let reader = std::io::BufReader::new(file);
        let mut blocks = Vec::new();
        for line in reader.lines().flatten() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(block) = serde_json::from_str::<LedgerBlock>(line) {
                blocks.push(block);
            }
        }
        let start = offset.min(blocks.len());
        let end = limit.map(|l| (start + l).min(blocks.len())).unwrap_or(blocks.len());
        blocks[start..end].to_vec()
    }

    pub fn height(&self) -> u64 {
        self.last_block().map(|b| b.index + 1).unwrap_or(0)
    }

    pub fn latest_hash(&self) -> String {
        self.last_block().map(|b| b.hash).unwrap_or_else(|| "0".repeat(64))
    }

    pub fn verify_chain(&self) -> Result<bool> {
        let chain = self.get_chain(None, 0);
        for window in chain.windows(2) {
            let prev = &window[0];
            let curr = &window[1];
            if curr.previous_hash != prev.hash {
                return Ok(false);
            }
            let mut check = curr.clone();
            check.hash = String::new();
            let recomputed = Self::compute_hash(&check);
            if recomputed != curr.hash {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Convenience: record a file event into both SQLite and the MembraLedger.
pub fn record_file_event(
    conn: &rusqlite::Connection,
    ledger: &MembraLedger,
    file_id: i64,
    path: &str,
    event_type: &str,
    notes: &str,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp() as f64;
    conn.execute(
        "INSERT INTO file_events (timestamp, file_id, path, event_type, notes, source) VALUES (?1, ?2, ?3, ?4, ?5, 'membra')",
        rusqlite::params![now, file_id, path, event_type, notes],
    )?;

    let file_id_str = file_id.to_string();
    let mut data = HashMap::new();
    data.insert("type", event_type);
    data.insert("path", path);
    data.insert("notes", notes);
    data.insert("file_id", &file_id_str);
    ledger.append(serde_json::to_value(data)?)?;
    Ok(())
}
