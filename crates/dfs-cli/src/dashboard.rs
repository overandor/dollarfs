use anyhow::Result;
use axum::{
    extract::Query,
    response::Html,
    routing::get,
    Json, Router,
};
use dfs_core::ledger::MembraLedger;
use rusqlite::{params, Connection};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
    ledger: MembraLedger,
}

pub async fn run_dashboard(db_path: PathBuf, config_dir: PathBuf, port: u16) -> Result<()> {
    let ledger = MembraLedger::from_config_dir(&config_dir);
    let state = Arc::new(AppState { db_path, ledger });

    let static_dir = config_dir.join("static");
    std::fs::create_dir_all(&static_dir)?;
    write_static_files(&static_dir)?;

    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/files", get(api_files))
        .route("/api/events", get(api_events))
        .route("/api/ledger", get(api_ledger))
        .route("/api/security", get(api_security))
        .route("/api/top", get(api_top))
        .route("/api/commits", get(api_commits))
        .nest_service("/static", ServeDir::new(static_dir))
        .route("/", get(index_handler))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    println!("lfv DASHBOARD — http://{}", addr);
    println!("  Press Ctrl-C to stop.");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn open_db(db_path: &PathBuf) -> Result<Connection> {
    Ok(rusqlite::Connection::open(db_path)?)
}

// ------------------------------------------------------------------
// Static files
// ------------------------------------------------------------------

fn write_static_files(dir: &PathBuf) -> Result<()> {
    std::fs::write(dir.join("style.css"), include_str!("../static/style.css"))?;
    std::fs::write(dir.join("app.js"), include_str!("../static/app.js"))?;
    Ok(())
}

async fn index_handler() -> Html<String> {
    Html(include_str!("../static/index.html").to_string())
}

// ------------------------------------------------------------------
// API handlers
// ------------------------------------------------------------------

#[derive(Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Deserialize)]
struct CommitsQuery {
    dir: String,
}

async fn api_status(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, String> {
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;

    let file_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM files WHERE deleted_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let total_value: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(book_value_usd), 0) FROM valuations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    let sec_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM security_findings", [], |row| row.get(0))
        .unwrap_or(0);

    let ledger_height = state.ledger.height();
    let latest_hash = state.ledger.latest_hash();

    Ok(Json(json!({
        "files": file_count,
        "total_value": total_value,
        "security_findings": sec_count,
        "ledger": {
            "block_height": ledger_height,
            "latest_hash": latest_hash,
        }
    })))
}

async fn api_files(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    let limit = q.limit.unwrap_or(50);

    let mut stmt = conn
        .prepare(
            "SELECT f.path, f.size, f.extension, f.mime_guess, f.indexed_at,
                    v.book_value_usd, v.valuation_confidence
             FROM files f
             LEFT JOIN valuations v ON v.file_id = f.file_id
             WHERE f.deleted_at IS NULL
             ORDER BY f.last_seen_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(json!({
                "path": row.get::<_, String>(0)?,
                "size": row.get::<_, i64>(1)?,
                "extension": row.get::<_, Option<String>>(2)?,
                "mime": row.get::<_, Option<String>>(3)?,
                "indexed_at": row.get::<_, f64>(4)?,
                "value": row.get::<_, Option<f64>>(5)?,
                "confidence": row.get::<_, Option<f64>>(6)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let files: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    Ok(Json(json!({ "files": files })))
}

async fn api_events(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    let limit = q.limit.unwrap_or(100);
    let offset = q.offset.unwrap_or(0);

    let mut stmt = conn
        .prepare(
            "SELECT timestamp, file_id, path, event_type, source, confidence, notes
             FROM file_events
             ORDER BY timestamp DESC
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit as i64, offset as i64], |row| {
            Ok(json!({
                "timestamp": row.get::<_, f64>(0)?,
                "file_id": row.get::<_, i64>(1)?,
                "path": row.get::<_, String>(2)?,
                "type": row.get::<_, String>(3)?,
                "source": row.get::<_, Option<String>>(4)?,
                "confidence": row.get::<_, Option<f64>>(5)?,
                "notes": row.get::<_, Option<String>>(6)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let events: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    Ok(Json(json!({ "events": events })))
}

async fn api_ledger(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let blocks = state.ledger.get_chain(q.limit, q.offset.unwrap_or(0));
    Ok(Json(json!({ "blocks": blocks })))
}

async fn api_security(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    let limit = q.limit.unwrap_or(100);

    let mut stmt = conn
        .prepare(
            "SELECT f.path, sf.line_number, sf.finding_type, sf.severity, sf.match_text
             FROM security_findings sf
             JOIN files f ON f.file_id = sf.file_id
             ORDER BY sf.severity DESC, f.path
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            let path: String = row.get(0)?;
            let line: Option<i64> = row.get(1)?;
            let finding_type: String = row.get(2)?;
            let severity: String = row.get(3)?;
            let match_text: Option<String> = row.get(4)?;
            let preview = match_text
                .as_deref()
                .map(dfs_security::redact_preview)
                .unwrap_or_default();
            Ok(json!({
                "path": path,
                "line": line.unwrap_or(0),
                "finding_type": finding_type,
                "severity": severity,
                "preview": preview,
            }))
        })
        .map_err(|e| e.to_string())?;

    let findings: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    Ok(Json(json!({ "findings": findings })))
}

async fn api_top(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Query(q): Query<LimitQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let conn = open_db(&state.db_path).map_err(|e| e.to_string())?;
    let limit = q.limit.unwrap_or(20);

    let mut stmt = conn
        .prepare(
            "SELECT f.path, v.book_value_usd, v.valuation_confidence, v.valuation_reason
             FROM valuations v
             JOIN files f ON f.file_id = v.file_id
             ORDER BY v.book_value_usd DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(json!({
                "path": row.get::<_, String>(0)?,
                "value": row.get::<_, f64>(1)?,
                "confidence": row.get::<_, f64>(2)?,
                "reason": row.get::<_, Option<String>>(3)?,
            }))
        })
        .map_err(|e| e.to_string())?;

    let top: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    Ok(Json(json!({ "top": top })))
}

async fn api_commits(
    axum::extract::State(_state): axum::extract::State<Arc<AppState>>,
    Query(q): Query<CommitsQuery>,
) -> Result<Json<serde_json::Value>, String> {
    let dir = std::path::Path::new(&q.dir);
    let mut commits = Vec::new();

    if dir.exists() {
        let output = std::process::Command::new("git")
            .args(["log", "-20", "--pretty=format:%H|%ci|%s"])
            .current_dir(dir)
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let text = String::from_utf8_lossy(&out.stdout);
                for line in text.lines() {
                    let parts: Vec<_> = line.splitn(3, '|').collect();
                    if parts.len() == 3 {
                        commits.push(json!({
                            "hash": &parts[0][..12.min(parts[0].len())],
                            "full_hash": parts[0],
                            "timestamp": parts[1],
                            "message": parts[2],
                        }));
                    }
                }
            }
        }
    }

    Ok(Json(json!({ "commits": commits })))
}
