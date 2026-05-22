use anyhow::Result;
use dfs_core::models::FileRecord;
use rusqlite::{params, Connection};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ValuationConfig {
    pub hourly_rate_usd: f64,
    pub llm_multiplier: f64,
    pub rnd_multiplier: f64,
    pub production_multiplier: f64,
    pub documentation_multiplier: f64,
    pub test_multiplier: f64,
    pub security_penalty: f64,
    pub duplicate_penalty: f64,
    pub unknown_confidence_discount: f64,
}

impl Default for ValuationConfig {
    fn default() -> Self {
        Self {
            hourly_rate_usd: 150.0,
            llm_multiplier: 0.35,
            rnd_multiplier: 2.0,
            production_multiplier: 2.5,
            documentation_multiplier: 1.2,
            test_multiplier: 1.3,
            security_penalty: 0.5,
            duplicate_penalty: 0.1,
            unknown_confidence_discount: 0.6,
        }
    }
}

impl ValuationConfig {
    pub fn load_from_db(conn: &Connection) -> Result<Self> {
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut values: HashMap<String, String> = HashMap::new();
        for row in rows {
            let (k, v) = row?;
            values.insert(k, v);
        }

        let parse = |key: &str, default: f64| -> f64 {
            values.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
        };

        Ok(Self {
            hourly_rate_usd: parse("hourly_rate_usd", 150.0),
            llm_multiplier: parse("llm_multiplier", 0.35),
            rnd_multiplier: parse("rnd_multiplier", 2.0),
            production_multiplier: parse("production_multiplier", 2.5),
            documentation_multiplier: parse("documentation_multiplier", 1.2),
            test_multiplier: parse("test_multiplier", 1.3),
            security_penalty: parse("security_penalty", 0.5),
            duplicate_penalty: parse("duplicate_penalty", 0.1),
            unknown_confidence_discount: parse("unknown_confidence_discount", 0.6),
        })
    }
}

pub fn value_file(conn: &Connection, file_id: i64, config: &ValuationConfig) -> Result<f64> {
    let file: FileRecord = conn.query_row(
        "SELECT file_id, path, canonical_path, inode, volume_id, content_hash, size,
                extension, mime_guess, created_at, modified_at, indexed_at,
                last_seen_at, deleted_at, duplicate_group_id, project_id, asset_id
         FROM files WHERE file_id = ?1",
        params![file_id],
        |row| {
            Ok(FileRecord {
                file_id: row.get(0)?,
                path: row.get(1)?,
                canonical_path: row.get(2)?,
                inode: row.get(3)?,
                volume_id: row.get(4)?,
                content_hash: row.get(5)?,
                size: row.get(6)?,
                extension: row.get(7)?,
                mime_guess: row.get(8)?,
                created_at: row.get(9)?,
                modified_at: row.get(10)?,
                indexed_at: row.get(11)?,
                last_seen_at: row.get(12)?,
                deleted_at: row.get(13)?,
                duplicate_group_id: row.get(14)?,
                project_id: row.get(15)?,
                asset_id: row.get(16)?,
            })
        },
    )?;

    let (direct_work_value, complexity_score, file_type_score) =
        estimate_file_type_value(&file, config);

    let has_security = conn.query_row(
        "SELECT COUNT(*) FROM security_findings WHERE file_id = ?1",
        params![file_id],
        |row| row.get::<_, i64>(0),
    )?;

    let is_duplicate = file.duplicate_group_id.is_some();

    let mut book_value = direct_work_value
        + (complexity_score * file_type_score)
        + (file.size as f64 * 0.0001);

    // Apply multipliers / penalties
    if is_duplicate {
        book_value *= config.duplicate_penalty;
    }
    if has_security > 0 {
        book_value *= config.security_penalty;
    }

    let rnd_bonus = if is_rnd_file(&file) {
        book_value * config.rnd_multiplier
    } else {
        0.0
    };

    let confidence = if file.mime_guess.is_some() {
        0.75
    } else {
        config.unknown_confidence_discount
    };

    let final_value = (book_value + rnd_bonus) * confidence;

    let reason = format!(
        "{} file, size={}, complexity_score={:.2}, confidence={:.2}, security_findings={}, duplicate={}",
        file.extension.as_deref().unwrap_or("unknown"),
        file.size,
        complexity_score,
        confidence,
        has_security,
        is_duplicate,
    );

    conn.execute(
        r#"INSERT INTO valuations (
            file_id, book_value_usd, replacement_cost_usd, rnd_value_usd,
            packaging_value_usd, market_confidence, valuation_confidence,
            last_valued_at, valuation_reason
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, unixepoch(), ?8)
        ON CONFLICT(file_id) DO UPDATE SET
            book_value_usd = excluded.book_value_usd,
            replacement_cost_usd = excluded.replacement_cost_usd,
            rnd_value_usd = excluded.rnd_value_usd,
            packaging_value_usd = excluded.packaging_value_usd,
            market_confidence = excluded.market_confidence,
            valuation_confidence = excluded.valuation_confidence,
            last_valued_at = excluded.last_valued_at,
            valuation_reason = excluded.valuation_reason"#,
        params![
            file_id,
            final_value,
            direct_work_value * 1.5,
            rnd_bonus,
            final_value * 0.8,
            confidence,
            confidence,
            reason,
        ],
    )?;

    Ok(final_value)
}

fn estimate_file_type_value(file: &FileRecord, config: &ValuationConfig) -> (f64, f64, f64) {
    let ext = file.extension.as_deref().unwrap_or("");
    let minutes_estimated = file.size as f64 / 500.0; // rough heuristic

    let (base_score, complexity) = match ext {
        "rs" | "swift" | "c" | "cpp" | "h" | "hpp" | "cc" => {
            (config.hourly_rate_usd * (minutes_estimated / 60.0).max(0.1), 1.5)
        }
        "py" | "js" | "ts" | "tsx" | "jsx" | "go" => {
            (config.hourly_rate_usd * (minutes_estimated / 60.0).max(0.05), 1.2)
        }
        "md" | "txt" | "rst" => {
            (config.hourly_rate_usd * (minutes_estimated / 60.0).max(0.02), 0.8)
        }
        "json" | "toml" | "yaml" | "yml" | "sql" | "sh" => {
            (config.hourly_rate_usd * (minutes_estimated / 60.0).max(0.01), 0.6)
        }
        "html" | "css" | "svg" => {
            (config.hourly_rate_usd * (minutes_estimated / 60.0).max(0.03), 0.7)
        }
        "pdf" => (5.0, 0.4),
        "jpg" | "jpeg" | "png" | "gif" | "webp" => (0.5, 0.1),
        _ => (0.1, 0.1),
    };

    let file_type_score = match file.mime_guess.as_deref() {
        Some("text/x-rust") | Some("text/x-c") | Some("text/x-c++") | Some("text/x-swift") => 1.5,
        Some("text/x-python") | Some("text/javascript") | Some("text/typescript") => 1.2,
        Some("text/markdown") => 1.0,
        Some("application/json") | Some("text/x-toml") | Some("text/x-yaml") => 0.8,
        _ => 0.5,
    };

    (base_score, complexity, file_type_score)
}

fn is_rnd_file(file: &FileRecord) -> bool {
    let path_lower = file.path.to_lowercase();
    let keywords = [
        "membra", "bearinglessfull", "semantic", "protocol", "runtime",
        "prod-llm-os", "dollarfs", "rnd", "research", "experiment",
        "model", "checkpoint", "training", "fine-tuned",
    ];
    keywords.iter().any(|kw| path_lower.contains(kw))
}

pub fn value_all_files(conn: &mut Connection, config: &ValuationConfig) -> Result<(usize, f64)> {
    let mut stmt = conn.prepare("SELECT file_id FROM files WHERE deleted_at IS NULL")?;
    let ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let mut total = 0.0;
    for id in &ids {
        match value_file(conn, *id, config) {
            Ok(v) => total += v,
            Err(e) => eprintln!("warn: valuation failed for file_id {}: {}", id, e),
        }
    }

    Ok((ids.len(), total))
}

pub fn value_directory(conn: &mut Connection, dir: &str, config: &ValuationConfig) -> Result<(usize, f64)> {
    let mut stmt = conn.prepare(
        "SELECT file_id FROM files WHERE path LIKE ?1 AND deleted_at IS NULL",
    )?;
    let ids: Vec<i64> = stmt
        .query_map(params![format!("{}%", dir)], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    let mut total = 0.0;
    for id in &ids {
        match value_file(conn, *id, config) {
            Ok(v) => total += v,
            Err(e) => eprintln!("warn: valuation failed for file_id {}: {}", id, e),
        }
    }

    Ok((ids.len(), total))
}

pub fn top_files(conn: &Connection, limit: usize) -> Result<Vec<(String, f64, f64, String)>> {
    let mut stmt = conn.prepare(
        r#"SELECT f.path, v.book_value_usd, v.valuation_confidence, v.valuation_reason
           FROM valuations v
           JOIN files f ON v.file_id = f.file_id
           WHERE f.deleted_at IS NULL
           ORDER BY v.book_value_usd DESC
           LIMIT ?1"#,
    )?;

    let rows = stmt.query_map(params![limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn day_ledger(conn: &mut Connection) -> Result<DailySummary> {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();

    let files_created: i64 = conn.query_row(
        "SELECT COUNT(*) FROM file_events WHERE event_type = 'file_created' AND date(timestamp, 'unixepoch') = ?1",
        params![&today],
        |row| row.get(0),
    )?;

    let files_modified: i64 = conn.query_row(
        "SELECT COUNT(*) FROM file_events WHERE event_type = 'file_modified' AND date(timestamp, 'unixepoch') = ?1",
        params![&today],
        |row| row.get(0),
    )?;

    let total_value: f64 = conn.query_row(
        "SELECT COALESCE(SUM(book_value_usd), 0) FROM valuations WHERE date(last_valued_at, 'unixepoch') = ?1",
        params![&today],
        |row| row.get(0),
    )?;

    let net_value: f64 = conn.query_row(
        "SELECT COALESCE(SUM(book_value_usd * COALESCE(valuation_confidence, 0.5)), 0) FROM valuations WHERE date(last_valued_at, 'unixepoch') = ?1",
        params![&today],
        |row| row.get(0),
    )?;

    let risk_deductions: f64 = conn.query_row(
        "SELECT COALESCE(SUM(book_value_usd * 0.5), 0) FROM valuations v JOIN security_findings s ON v.file_id = s.file_id WHERE date(v.last_valued_at, 'unixepoch') = ?1",
        params![&today],
        |row| row.get(0),
    )?;

    let top = top_files(conn, 10)?;

    conn.execute(
        r#"INSERT INTO daily_ledgers (
            date, files_created, files_modified, estimated_gross_value,
            estimated_net_value, risk_deductions, top_10_files
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        ON CONFLICT(date) DO UPDATE SET
            files_created = excluded.files_created,
            files_modified = excluded.files_modified,
            estimated_gross_value = excluded.estimated_gross_value,
            estimated_net_value = excluded.estimated_net_value,
            risk_deductions = excluded.risk_deductions,
            top_10_files = excluded.top_10_files"#,
        params![
            &today,
            files_created,
            files_modified,
            total_value,
            net_value,
            risk_deductions,
            serde_json::to_string(&top.iter().map(|(p, v, c, _)| (p.clone(), *v, *c)).collect::<Vec<_>>())?,
        ],
    )?;

    Ok(DailySummary {
        date: today,
        files_created,
        files_modified,
        estimated_gross_value: total_value,
        estimated_net_value: net_value,
        risk_deductions,
        top_files: top.into_iter().map(|(p, v, c, r)| (p, v, c, r)).collect(),
    })
}

#[derive(Debug)]
pub struct DailySummary {
    pub date: String,
    pub files_created: i64,
    pub files_modified: i64,
    pub estimated_gross_value: f64,
    pub estimated_net_value: f64,
    pub risk_deductions: f64,
    pub top_files: Vec<(String, f64, f64, String)>,
}
