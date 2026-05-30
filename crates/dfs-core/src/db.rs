use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub fn init_db(path: &Path) -> Result<Connection> {
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    let conn = Connection::open(path)?;

    conn.execute_batch(SCHEMA)?;
    run_migrations(&conn)?;
    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<()> {
    // Migration: add entropy column if missing
    let has_entropy: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name = 'entropy'",
        [],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) != 0;
    if !has_entropy {
        conn.execute("ALTER TABLE files ADD COLUMN entropy REAL DEFAULT 0.0", [])?;
    }

    // Migration: add is_sparse column if missing
    let has_sparse: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name = 'is_sparse'",
        [],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) != 0;
    if !has_sparse {
        conn.execute("ALTER TABLE files ADD COLUMN is_sparse INTEGER DEFAULT 0", [])?;
    }

    // Migration: add schema_version column if missing
    let has_schema_version: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('valuations') WHERE name = 'schema_version'",
        [],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) != 0;
    if !has_schema_version {
        conn.execute("ALTER TABLE valuations ADD COLUMN schema_version TEXT DEFAULT '0.2.0'", [])?;
    }

    // Migration: add is_legacy column if missing
    let has_legacy: bool = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('valuations') WHERE name = 'is_legacy'",
        [],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) != 0;
    if !has_legacy {
        conn.execute("ALTER TABLE valuations ADD COLUMN is_legacy INTEGER DEFAULT 1", [])?;
    }

    // Mark all pre-0.3.0 valuations as legacy
    conn.execute(
        "UPDATE valuations SET is_legacy = 1 WHERE schema_version IS NULL OR schema_version < '0.3.0'",
        [],
    )?;

    Ok(())
}

const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS files (
    file_id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    canonical_path TEXT,
    inode INTEGER,
    volume_id TEXT,
    content_hash TEXT NOT NULL,
    size INTEGER NOT NULL DEFAULT 0,
    extension TEXT,
    mime_guess TEXT,
    created_at REAL,
    modified_at REAL,
    indexed_at REAL NOT NULL DEFAULT (unixepoch()),
    last_seen_at REAL NOT NULL DEFAULT (unixepoch()),
    deleted_at REAL,
    duplicate_group_id INTEGER,
    project_id INTEGER,
    asset_id TEXT,
    entropy REAL DEFAULT 0.0,
    is_sparse INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS file_events (
    event_id INTEGER PRIMARY KEY AUTOINCREMENT,
    timestamp REAL NOT NULL DEFAULT (unixepoch()),
    file_id INTEGER NOT NULL,
    path TEXT NOT NULL,
    event_type TEXT NOT NULL,
    process_name TEXT,
    source TEXT,
    confidence REAL,
    before_hash TEXT,
    after_hash TEXT,
    valuation_delta_usd REAL,
    notes TEXT,
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE TABLE IF NOT EXISTS valuations (
    valuation_id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL UNIQUE,
    book_value_usd REAL,
    replacement_cost_usd REAL,
    rnd_value_usd REAL,
    packaging_value_usd REAL,
    market_confidence REAL,
    valuation_confidence REAL,
    last_valued_at REAL NOT NULL DEFAULT (unixepoch()),
    valuation_reason TEXT,
    schema_version TEXT DEFAULT '0.3.0',
    is_legacy INTEGER DEFAULT 0,
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE TABLE IF NOT EXISTS projects (
    project_id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    root_path TEXT NOT NULL,
    created_at REAL NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE IF NOT EXISTS llm_attribution (
    attribution_id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    confidence TEXT NOT NULL DEFAULT 'unknown',
    tagged_at REAL NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE TABLE IF NOT EXISTS security_findings (
    finding_id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    finding_type TEXT NOT NULL,
    line_number INTEGER,
    match_text TEXT,
    severity TEXT NOT NULL DEFAULT 'medium',
    detected_at REAL NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE TABLE IF NOT EXISTS evidence_cards (
    card_id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    summary TEXT,
    project_id INTEGER,
    rnd_qualification TEXT,
    estimated_value REAL,
    valuation_confidence TEXT,
    llm_attribution TEXT,
    commits TEXT,
    tests TEXT,
    risks TEXT,
    proof_notes TEXT,
    next_action TEXT,
    created_at REAL NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE TABLE IF NOT EXISTS daily_ledgers (
    ledger_id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL UNIQUE,
    files_created INTEGER DEFAULT 0,
    files_modified INTEGER DEFAULT 0,
    repos_touched INTEGER DEFAULT 0,
    commits_detected INTEGER DEFAULT 0,
    tests_run INTEGER DEFAULT 0,
    exports_created INTEGER DEFAULT 0,
    docs_written INTEGER DEFAULT 0,
    llm_sessions_imported INTEGER DEFAULT 0,
    estimated_gross_value REAL DEFAULT 0.0,
    estimated_net_value REAL DEFAULT 0.0,
    risk_deductions REAL DEFAULT 0.0,
    top_10_files TEXT,
    top_10_increases TEXT,
    top_10_liabilities TEXT
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tags (
    tag_id INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id INTEGER NOT NULL,
    tag TEXT NOT NULL,
    tagged_at REAL NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (file_id) REFERENCES files(file_id)
);

CREATE INDEX IF NOT EXISTS idx_files_hash ON files(content_hash);
CREATE INDEX IF NOT EXISTS idx_files_path ON files(path);
CREATE INDEX IF NOT EXISTS idx_files_modified ON files(modified_at);
CREATE INDEX IF NOT EXISTS idx_files_deleted ON files(deleted_at);
CREATE INDEX IF NOT EXISTS idx_events_file ON file_events(file_id);
CREATE INDEX IF NOT EXISTS idx_events_type ON file_events(event_type);
CREATE INDEX IF NOT EXISTS idx_valuations_file ON valuations(file_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_valuations_file_unique ON valuations(file_id);
CREATE INDEX IF NOT EXISTS idx_security_file ON security_findings(file_id);
"#;

pub fn ensure_settings(conn: &mut Connection) -> Result<()> {
    let defaults = [
        ("hourly_rate_usd", "150"),
        ("llm_multiplier", "0.35"),
        ("rnd_multiplier", "2.0"),
        ("production_multiplier", "2.5"),
        ("documentation_multiplier", "1.2"),
        ("test_multiplier", "1.3"),
        ("security_penalty", "0.5"),
        ("duplicate_penalty", "0.1"),
        ("unknown_confidence_discount", "0.6"),
        ("version", "0.3.0"),
        ("initialized_at", &format!("{}", chrono::Utc::now().timestamp())),
        ("llm_enabled", "false"),
        ("llm_endpoint", "http://localhost:11434/v1/chat/completions"),
        ("llm_model", "llama3.1"),
        ("llm_api_key", ""),
        ("llm_timeout", "120"),
        ("llm_max_tokens", "2048"),
        ("llm_temperature", "0.3"),
    ];
    for (k, v) in &defaults {
        conn.execute(
            "INSERT OR IGNORE INTO settings(key, value) VALUES (?1, ?2)",
            rusqlite::params![k, v],
        )?;
    }
    Ok(())
}
