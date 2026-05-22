mod tui;

use anyhow::Result;
use chrono::Local;
use clap::{Parser, Subcommand};
use dfs_core::{scanner, DollarFs, default_watched_dirs};
use dfs_security::{redact_preview, scan_directory};
use dfs_valuation::{day_ledger, top_files, value_all_files, value_directory, value_file, ValuationConfig};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Parser)]
#[command(name = "lfv")]
#[command(about = "lfv — local-first macOS file-value terminal")]
#[command(version = "0.2.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, help = "Configuration directory")]
    config: Option<PathBuf>,

    #[arg(short, long, global = true, help = "Verbose output")]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize lfv database and config
    Init,
    /// Scan a directory tree and index files
    Scan {
        #[arg(help = "Directory to scan")]
        path: PathBuf,
        #[arg(short, long, help = "Only index changed files")]
        incremental: bool,
    },
    /// Show estimated dollar value of files
    Value {
        #[arg(help = "Path to value")]
        path: Option<PathBuf>,
    },
    /// List top valuable files
    Top {
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Show today's work ledger
    Day,
    /// Scan for secrets and credentials
    Secrets {
        #[arg(help = "Directory to scan")]
        path: Option<PathBuf>,
        #[arg(short, long, help = "Show detailed findings with previews")]
        detail: bool,
    },
    /// Export reports
    Export {
        #[arg(help = "Export format: markdown, json, csv")]
        format: String,
        #[arg(short, long, help = "Output file path")]
        output: Option<PathBuf>,
    },
    /// Watch directories for file changes
    Watch {
        #[arg(help = "Directories to watch")]
        paths: Vec<PathBuf>,
    },
    /// Detect and group duplicate files
    Duplicates,
    /// Run 24/7 LLM agent for continuous file analysis
    Agent {
        #[arg(short, long, default_value = "300", help = "Analysis interval in seconds")]
        interval: u64,
    },
    /// Analyze a single file with the configured LLM
    Analyze {
        #[arg(help = "File path to analyze")]
        path: PathBuf,
    },
    /// Configure LLM endpoint and model
    LlmConfig {
        #[arg(short, long, help = "LLM API endpoint URL")]
        endpoint: Option<String>,
        #[arg(short, long, help = "Model name")]
        model: Option<String>,
        #[arg(short, long, help = "API key")]
        api_key: Option<String>,
        #[arg(long, help = "Enable LLM integration")]
        enable: bool,
        #[arg(long, help = "Disable LLM integration")]
        disable: bool,
    },
    /// Open terminal UI dashboard
    Tui,
    /// Show system status
    Status,
    /// Run diagnostics
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .compact()
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    match cli.command {
        Commands::Init => cmd_init(cli.config).await,
        Commands::Scan { path, incremental } => cmd_scan(cli.config, &path, incremental).await,
        Commands::Value { path } => cmd_value(cli.config, path.as_deref()).await,
        Commands::Top { limit } => cmd_top(cli.config, limit).await,
        Commands::Day => cmd_day(cli.config).await,
        Commands::Secrets { path, detail } => cmd_secrets(cli.config, path.as_deref(), detail).await,
        Commands::Export { format, output } => cmd_export(cli.config, &format, output.as_deref()).await,
        Commands::Watch { paths } => cmd_watch(cli.config, &paths).await,
        Commands::Duplicates => cmd_duplicates(cli.config).await,
        Commands::Agent { interval } => cmd_agent(cli.config, interval).await,
        Commands::Analyze { path } => cmd_analyze(cli.config, &path).await,
        Commands::LlmConfig { endpoint, model, api_key, enable, disable } => {
            cmd_llm_config(cli.config, endpoint, model, api_key, enable, disable).await
        }
        Commands::Tui => cmd_tui().await,
        Commands::Status => cmd_status(cli.config).await,
        Commands::Doctor => cmd_doctor(cli.config).await,
    }
}

async fn cmd_init(config: Option<PathBuf>) -> Result<()> {
    println!("lfv v0.2.0 — Initializing...");
    let dfs = DollarFs::init(config)?;
    println!("  Config directory: {}", dfs.config_dir.display());
    println!("  Database: {}", dfs.db_path.display());
    println!("  Settings: default valuation config loaded");
    println!("Ready. Run `lfv scan <path>` to begin.");
    Ok(())
}

async fn cmd_scan(config: Option<PathBuf>, path: &std::path::Path, incremental: bool) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    println!("Scanning: {}", abs.display());
    if incremental {
        println!("  (incremental mode — only changed files)");
    }

    let mut conn = dfs.open_db()?;
    let count = scanner::scan_path_incremental(&mut conn, &abs, incremental)?;
    println!("  Indexed {} files", count);

    let config = ValuationConfig::load_from_db(&conn)?;
    let (valued, total) = value_all_files(&mut conn, &config)?;
    println!("  Valued {} files, total book value: ${:.2}", valued, total);

    info!("scan complete: {} files, ${:.2} value", count, total);
    Ok(())
}

async fn cmd_value(config: Option<PathBuf>, path: Option<&std::path::Path>) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let mut conn = dfs.open_db()?;
    let vconfig = ValuationConfig::load_from_db(&conn)?;

    if let Some(p) = path {
        let abs = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        let path_str = abs.to_string_lossy().to_string();
        if abs.is_file() {
            // Single file detailed report
            let file_id: Option<i64> = conn.query_row(
                "SELECT file_id FROM files WHERE path = ?1 AND deleted_at IS NULL",
                rusqlite::params![&path_str],
                |row| row.get(0),
            ).ok();
            if let Some(fid) = file_id {
                let _value = value_file(&mut conn, fid, &vconfig)?;
                let (book, conf, reason, sec_count): (f64, f64, String, i64) = conn.query_row(
                    "SELECT book_value_usd, valuation_confidence, valuation_reason,
                            (SELECT COUNT(*) FROM security_findings WHERE file_id = ?1)
                     FROM valuations WHERE file_id = ?1",
                    rusqlite::params![fid],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )?;
                let conf_label = if conf >= 0.8 { "High" } else if conf >= 0.5 { "Medium" } else { "Low" };
                let sec_label = if sec_count > 0 { format!("{} findings", sec_count) } else { "Pass".to_string() };
                println!("FILE VALUE REPORT");
                println!("Path: {}", path_str);
                println!("Book value: ${:.0}", book);
                println!("Confidence: {}", conf_label);
                println!("Security: {}", sec_label);
                println!("Reason: {}", reason);
            } else {
                println!("File not indexed: {}", path_str);
                println!("Run `lfv scan {}` first.", p.parent().unwrap_or(p).display());
            }
        } else {
            let (count, total) = value_directory(&mut conn, &path_str, &vconfig)?;
            println!("lfv VALUE REPORT");
            println!("Path: {}", path_str);
            println!("Files: {}", count);
            println!("Book value: ${:.2}", total);
        }
        println!("\nNote: Dollar values are internal estimates for accountability and prioritization.");
        println!("They are not guaranteed market prices or resale values.");
    } else {
        let (count, total) = value_all_files(&mut conn, &vconfig)?;
        println!("lfv VALUE REPORT — ALL FILES");
        println!("Total files: {}", count);
        println!("Total book value: ${:.2}", total);
    }
    Ok(())
}

async fn cmd_top(config: Option<PathBuf>, limit: usize) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let conn = dfs.open_db()?;
    let top = top_files(&conn, limit)?;

    println!("lfv TOP FILES");
    println!("{:<6} {:<50} {:>12} {:>10}", "RANK", "PATH", "VALUE", "CONF");
    for (i, (path, value, conf, _reason)) in top.iter().enumerate() {
        let conf_label = if *conf >= 0.8 {
            "High"
        } else if *conf >= 0.5 {
            "Medium"
        } else {
            "Low"
        };
        println!(
            "{:<6} {:<50} ${:>10.2} {:>10}",
            i + 1,
            truncate(path, 50),
            value,
            conf_label
        );
    }
    println!("\nNote: Values are internal estimates, not guaranteed market prices.");
    Ok(())
}

async fn cmd_day(config: Option<PathBuf>) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let mut conn = dfs.open_db()?;
    let summary = day_ledger(&mut conn)?;

    println!("lfv WORK LEDGER — {}", summary.date);
    println!("  Files created:    {}", summary.files_created);
    println!("  Files modified:   {}", summary.files_modified);
    println!("  Gross value:      ${:.2}", summary.estimated_gross_value);
    println!("  Net value:        ${:.2}", summary.estimated_net_value);
    println!("  Risk deductions:  ${:.2}", summary.risk_deductions);

    if !summary.top_files.is_empty() {
        println!("\n  Top value creators:");
        for (path, value, conf, _) in summary.top_files.iter().take(5) {
            println!("    {:<45} ${:>8.2}  (conf: {:.0}%)", truncate(path, 45), value, conf * 100.0);
        }
    }
    println!("\nNote: Dollar values are internal estimates for accountability.");
    Ok(())
}

async fn cmd_secrets(
    config: Option<PathBuf>,
    path: Option<&std::path::Path>,
    detail: bool,
) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let mut conn = dfs.open_db()?;

    let dir = path
        .map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join("Developer"))
                .unwrap_or_else(|| PathBuf::from("."))
        });
    let dir_str = dir.to_string_lossy().to_string();

    println!("lfv SECURITY SCAN — {}", dir_str);
    let mut findings = scan_directory(&mut conn, &dir_str)?;
    if findings.is_empty() {
        findings = dfs_security::scan_directory_raw(&dir)?;
    }

    if findings.is_empty() {
        println!("  No secrets detected.");
        return Ok(());
    }

    if detail {
        println!("  {} findings:", findings.len());
        for f in findings.iter().take(40) {
            let sev_prefix = match f.severity.as_str() {
                "critical" => "[CRIT]  ",
                "high" => "[HIGH]  ",
                _ => "[MED]   ",
            };
            let truncated_path = if f.path.len() > 45 {
                format!("{}...", &f.path[..42])
            } else {
                f.path.clone()
            };
            println!(
                "    {}{:<50}  line {:>4}  {}  {}",
                sev_prefix, truncated_path, f.line_number, f.finding_type, f.preview
            );
        }
        if findings.len() > 40 {
            println!("    ... and {} more", findings.len() - 40);
        }
    } else {
        use std::collections::HashMap;
        let mut by_file: HashMap<String, usize> = HashMap::new();
        for f in &findings {
            *by_file.entry(f.path.clone()).or_insert(0) += 1;
        }
        let mut files: Vec<_> = by_file.iter().collect();
        files.sort_by(|a, b| b.1.cmp(a.1));
        println!("  {} files with potential secrets:", files.len());
        for (path, count) in files.iter().take(20) {
            let truncated = if path.len() > 60 {
                format!("{}...", &path[..57])
            } else {
                path.to_string()
            };
            println!("    {}  ({} findings)", truncated, count);
        }
        if files.len() > 20 {
            println!("    ... and {} more", files.len() - 20);
        }
    }

    println!("\nWARNING: Redact secrets before committing. Never upload to cloud.");
    Ok(())
}

async fn cmd_watch(config: Option<PathBuf>, paths: &[PathBuf]) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let mut conn = dfs.open_db()?;

    let watch_paths: Vec<std::path::PathBuf> = if paths.is_empty() {
        default_watched_dirs()
    } else {
        paths.to_vec()
    };

    let path_refs: Vec<&std::path::Path> = watch_paths.iter().map(|p| p.as_path()).collect();
    dfs_core::watcher::run_watch_loop(&path_refs, &mut conn)?;
    Ok(())
}

async fn cmd_duplicates(config: Option<PathBuf>) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let mut conn = dfs.open_db()?;

    println!("lfv DUPLICATE DETECTION");
    let updated = scanner::detect_duplicates(&mut conn)?;
    println!("  {} duplicate file(s) grouped", updated);
    Ok(())
}

async fn cmd_agent(config: Option<PathBuf>, interval: u64) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let mut conn = dfs.open_db()?;
    let dirs = default_watched_dirs();
    let path_refs: Vec<&std::path::Path> = dirs.iter().map(|p| p.as_path()).collect();
    dfs_core::agent::run_agent(&mut conn, &path_refs, interval).await
}

async fn cmd_analyze(config: Option<PathBuf>, path: &std::path::Path) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let mut conn = dfs.open_db()?;
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    dfs_core::agent::analyze_single_file(&mut conn, &abs).await
}

async fn cmd_llm_config(
    config: Option<PathBuf>,
    endpoint: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    enable: bool,
    disable: bool,
) -> Result<()> {
    use dfs_core::llm::LlmConfig;
    let dfs = DollarFs::init(config)?;
    let conn = dfs.open_db()?;

    let mut cfg = LlmConfig::load_from_db(&conn)?;

    if let Some(e) = endpoint {
        cfg.endpoint = e;
    }
    if let Some(m) = model {
        cfg.model = m;
    }
    if let Some(k) = api_key {
        cfg.api_key = Some(k);
    }
    if enable {
        cfg.enabled = true;
    }
    if disable {
        cfg.enabled = false;
    }

    cfg.save_to_db(&conn)?;

    println!("lfv LLM CONFIG");
    println!("  Endpoint:    {}", cfg.endpoint);
    println!("  Model:       {}", cfg.model);
    println!("  API Key:     {}", if cfg.api_key.is_some() { "<set>" } else { "<none>" });
    println!("  Timeout:     {}s", cfg.timeout_seconds);
    println!("  Max Tokens:  {}", cfg.max_tokens);
    println!("  Temperature: {}", cfg.temperature);
    println!("  Enabled:     {}", cfg.enabled);
    println!("\nSave successful. Run `lfv agent` to start 24/7 analysis.");
    Ok(())
}

async fn cmd_export(
    config: Option<PathBuf>,
    format: &str,
    output: Option<&std::path::Path>,
) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let conn = dfs.open_db()?;
    let today = Local::now().format("%Y-%m-%d").to_string();

    match format {
        "markdown" => {
            let mut md = String::new();
            md.push_str(&format!("# lfv Report — {}\n\n", today));
            md.push_str("> This is a proprietary internal report.\n");
            md.push_str("> Dollar values are estimates for accountability, not guaranteed market prices.\n\n",
            );

            let top = top_files(&conn, 20)?;
            md.push_str("## Top Valuable Files\n\n");
            md.push_str("| Rank | Path | Value | Confidence |\n");
            md.push_str("|------|------|-------|------------|\n");
            for (i, (path, value, conf, _)) in top.iter().enumerate() {
                let conf_label = if *conf >= 0.8 {
                    "High"
                } else if *conf >= 0.5 {
                    "Medium"
                } else {
                    "Low"
                };
                md.push_str(&format!(
                    "| {} | {} | ${:.2} | {} |\n",
                    i + 1,
                    path,
                    value,
                    conf_label
                ));
            }

            // Security summary
            let sec_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM security_findings",
                [],
                |row| row.get(0),
            )?;
            md.push_str(&format!("\n## Security Summary\n\n"));
            md.push_str(&format!("Total findings: {}\n\n", sec_count));

            // Total value
            let total: f64 = conn.query_row(
                "SELECT COALESCE(SUM(book_value_usd), 0) FROM valuations",
                [],
                |row| row.get(0),
            )?;
            md.push_str(&format!("\n## Total Book Value\n\n"));
            md.push_str(&format!("${:.2}\n\n", total));
            md.push_str("*Internal estimate only. Not a market appraisal.*\n");

            let out_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(format!("lfv-report-{}.md", today))
            });
            std::fs::write(&out_path, md)?;
            println!("Report written to: {}", out_path.display());
        }
        "json" => {
            let top = top_files(&conn, 100)?;
            let report = serde_json::json!({
                "generated_at": today,
                "top_files": top.iter().map(|(p, v, c, r)| serde_json::json!({
                    "path": p, "value": v, "confidence": c, "reason": r
                })).collect::<Vec<_>>(),
                "note": "Internal estimates only. Not guaranteed market prices.",
            });
            let out_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(format!("lfv-report-{}.json", today))
            });
            std::fs::write(&out_path, serde_json::to_string_pretty(&report)?)?;
            println!("Report written to: {}", out_path.display());
        }
        "csv" => {
            let mut csv = String::new();
            csv.push_str("path,book_value_usd,valuation_confidence,valuation_reason\n");
            let mut stmt = conn.prepare(
                "SELECT f.path, v.book_value_usd, v.valuation_confidence, v.valuation_reason
                 FROM valuations v JOIN files f ON v.file_id = f.file_id
                 WHERE f.deleted_at IS NULL ORDER BY v.book_value_usd DESC"
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, f64>(1)?,
                    row.get::<_, f64>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            for row in rows {
                let (path, val, conf, reason) = row?;
                let safe_reason = reason.replace('"', "\"\"").replace('\n', " ");
                csv.push_str(&format!("\"{}\",{:.2},{:.2},\"{}\"\n", path, val, conf, safe_reason));
            }
            let out_path = output.map(|p| p.to_path_buf()).unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(format!("lfv-report-{}.csv", today))
            });
            std::fs::write(&out_path, csv)?;
            println!("Report written to: {}", out_path.display());
        }
        _ => {
            warn!("Unknown export format: {}", format);
            println!("Supported formats: markdown, json, csv");
        }
    }
    Ok(())
}

async fn cmd_tui() -> Result<()> {
    let dfs = DollarFs::init(None)?;
    let conn = dfs.open_db()?;

    let file_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE deleted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let total_value: f64 = conn.query_row(
        "SELECT COALESCE(SUM(book_value_usd), 0) FROM valuations",
        [],
        |row| row.get(0),
    )?;
    let sec_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM security_findings",
        [],
        |row| row.get(0),
    )?;
    let top = top_files(&conn, 50)?;

    let mut sec_stmt = conn.prepare(
        "SELECT f.path, sf.line_number, sf.finding_type, sf.severity, sf.match_text
         FROM security_findings sf
         JOIN files f ON f.file_id = sf.file_id
         ORDER BY sf.severity DESC, f.path"
    )?;
    let sec_rows = sec_stmt.query_map([], |row| {
        let path: String = row.get(0)?;
        let line: Option<i64> = row.get(1)?;
        let finding_type: String = row.get(2)?;
        let severity: String = row.get(3)?;
        let match_text: Option<String> = row.get(4)?;
        let preview = match_text.as_deref().map(redact_preview).unwrap_or_default();
        Ok((path, line.unwrap_or(0), finding_type, severity, preview))
    })?;
    let security_findings: Vec<_> = sec_rows.filter_map(|r| r.ok()).collect();

    tui::run_tui(file_count, total_value, sec_count, top, security_findings)?;
    Ok(())
}

async fn cmd_status(config: Option<PathBuf>) -> Result<()> {
    let dfs = DollarFs::init(config)?;
    let conn = dfs.open_db()?;

    let file_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE deleted_at IS NULL",
        [],
        |row| row.get(0),
    )?;
    let total_value: f64 = conn.query_row(
        "SELECT COALESCE(SUM(book_value_usd), 0) FROM valuations",
        [],
        |row| row.get(0),
    )?;
    let sec_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM security_findings",
        [],
        |row| row.get(0),
    )?;

    println!("lfv STATUS");
    println!("  Config dir:   {}", dfs.config_dir.display());
    println!("  Database:     {}", dfs.db_path.display());
    println!("  Files:        {}", file_count);
    println!("  Book value:   ${:.2}", total_value);
    println!("  Securities:   {}", sec_count);
    println!("  Version:      0.2.0");
    Ok(())
}

async fn cmd_doctor(config: Option<PathBuf>) -> Result<()> {
    println!("lfv DOCTOR");
    let dfs = DollarFs::init(config)?;
    let conn = dfs.open_db()?;

    println!("  [PASS] Database open: {}", dfs.db_path.display());

    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
    )?;
    let tables: Vec<String> = stmt.query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    println!("  [PASS] Tables: {}", tables.join(", "));

    let file_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE deleted_at IS NULL", [], |row| row.get(0),
    )?;
    println!("  [INFO] Indexed files: {}", file_count);

    let settings: i64 = conn.query_row(
        "SELECT COUNT(*) FROM settings", [], |row| row.get(0),
    )?;
    println!("  [INFO] Config entries: {}", settings);

    println!("\nDiagnostics complete.");
    Ok(())
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}
