use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub file_id: Option<i64>,
    pub path: String,
    pub canonical_path: Option<String>,
    pub inode: Option<i64>,
    pub volume_id: Option<String>,
    pub content_hash: String,
    pub size: i64,
    pub extension: Option<String>,
    pub mime_guess: Option<String>,
    pub created_at: Option<f64>,
    pub modified_at: Option<f64>,
    pub indexed_at: f64,
    pub last_seen_at: f64,
    pub deleted_at: Option<f64>,
    pub duplicate_group_id: Option<i64>,
    pub project_id: Option<i64>,
    pub asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Valuation {
    pub valuation_id: Option<i64>,
    pub file_id: i64,
    pub book_value_usd: Option<f64>,
    pub replacement_cost_usd: Option<f64>,
    pub rnd_value_usd: Option<f64>,
    pub packaging_value_usd: Option<f64>,
    pub market_confidence: Option<f64>,
    pub valuation_confidence: Option<f64>,
    pub last_valued_at: f64,
    pub valuation_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityFinding {
    pub finding_id: Option<i64>,
    pub file_id: i64,
    pub finding_type: String,
    pub line_number: Option<i64>,
    pub match_text: Option<String>,
    pub severity: String,
    pub detected_at: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyLedger {
    pub ledger_id: Option<i64>,
    pub date: String,
    pub files_created: i64,
    pub files_modified: i64,
    pub repos_touched: i64,
    pub commits_detected: i64,
    pub tests_run: i64,
    pub exports_created: i64,
    pub docs_written: i64,
    pub llm_sessions_imported: i64,
    pub estimated_gross_value: f64,
    pub estimated_net_value: f64,
    pub risk_deductions: f64,
}
