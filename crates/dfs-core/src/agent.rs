use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;
use std::time::Duration;

use crate::llm::{analyze_file_value, classify_llm_attribution, detect_secrets_llm, generate_evidence_card, LlmClient, LlmConfig};
use crate::scanner::is_excluded;

/// 24/7 agent that continuously analyzes files using an LLM.
/// Runs in a loop: discovers files, analyzes them, stores results, sleeps.
pub async fn run_agent(
    conn: &mut Connection,
    watched_dirs: &[&Path],
    interval_secs: u64,
) -> Result<()> {
    let config = LlmConfig::load_from_db(conn)?;
    if !config.enabled {
        println!("lfv AGENT");
        println!("  LLM is disabled. Run `lfv llm-config --enable` to start the agent.");
        return Ok(());
    }

    let client = LlmClient::new(config)?;

    // Verify connectivity
    print!("lfv AGENT — Checking LLM endpoint... ");
    match client.ping().await {
        Ok(resp) => println!("connected ({resp})"),
        Err(e) => {
            println!("FAILED: {e}");
            println!("  Check your endpoint and model. Run `lfv llm-config` to view settings.");
            return Ok(());
        }
    }

    println!("lfv AGENT — 24/7 file observability LLM running.");
    println!("  Watching dirs: {}", watched_dirs.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
    println!("  Analysis interval: {interval_secs}s");
    println!("  Press Ctrl-C to stop.\n");

    let interval = Duration::from_secs(interval_secs);

    loop {
        let start = std::time::Instant::now();
        let mut total_analyzed = 0usize;

        for dir in watched_dirs {
            if !dir.exists() {
                continue;
            }

            // Find files not yet analyzed by the agent
            let files = find_pending_files(conn, dir)?;
            for (file_id, path) in files {
                if is_excluded(Path::new(&path), crate::scanner::DEFAULT_EXCLUDED) {
                    continue;
                }

                // Skip binary/unreadable files
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                if content.len() < 10 {
                    continue;
                }

                println!("  analyzing: {}", path);

                // 1. Value analysis
                match analyze_file_value(&client, &path, &content).await {
                    Ok(analysis) => {
                        let _ = store_agent_event(conn, file_id, "value_analysis", &analysis);
                    }
                    Err(e) => tracing::warn!("value analysis failed for {}: {}", path, e),
                }

                // 2. LLM attribution
                match classify_llm_attribution(&client, &content).await {
                    Ok(attribution) => {
                        let _ = store_llm_attribution(conn, file_id, &attribution);
                    }
                    Err(e) => tracing::warn!("attribution failed for {}: {}", path, e),
                }

                // 3. Deep security scan via LLM
                match detect_secrets_llm(&client, &content).await {
                    Ok(findings) if findings != "NONE" && !findings.is_empty() => {
                        let _ = store_agent_event(conn, file_id, "llm_security_scan", &findings);
                    }
                    _ => {}
                }

                // 4. Evidence card (if file has valuation)
                let val: Option<f64> = conn
                    .query_row(
                        "SELECT book_value_usd FROM valuations WHERE file_id = ?1",
                        params![file_id],
                        |row| row.get(0),
                    )
                    .ok();
                let sec_count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM security_findings WHERE file_id = ?1",
                    params![file_id],
                    |row| row.get(0),
                )?;

                if let Some(v) = val {
                    match generate_evidence_card(&client, &path, &content, sec_count as usize, v).await {
                        Ok(card) => {
                            let _ = store_evidence_card(conn, file_id, &card);
                        }
                        Err(e) => tracing::warn!("evidence card failed for {}: {}", path, e),
                    }
                }

                total_analyzed += 1;

                // Brief yield to keep system responsive
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        let elapsed = start.elapsed();
        println!("  cycle complete: {total_analyzed} files analyzed in {:.1}s", elapsed.as_secs_f64());

        let sleep_remaining = interval.saturating_sub(elapsed);
        if sleep_remaining > Duration::ZERO {
            println!("  sleeping for {:.0}s...", sleep_remaining.as_secs_f64());
            tokio::time::sleep(sleep_remaining).await;
        }
    }
}

/// One-shot analysis of a single file.
pub async fn analyze_single_file(conn: &mut Connection, path: &Path) -> Result<()> {
    let config = LlmConfig::load_from_db(conn)?;
    if !config.enabled {
        println!("LLM is disabled. Enable with `lfv llm-config --enable`.");
        return Ok(());
    }
    let client = LlmClient::new(config)?;

    let content = std::fs::read_to_string(path)?;
    let path_str = path.to_string_lossy().to_string();

    println!("lfv ANALYZE — {}", path_str);

    let file_id: i64 = conn.query_row(
        "SELECT file_id FROM files WHERE path = ?1 AND deleted_at IS NULL",
        params![&path_str],
        |row| row.get(0),
    )?;

    print!("  value analysis... ");
    match analyze_file_value(&client, &path_str, &content).await {
        Ok(a) => {
            println!("done");
            store_agent_event(conn, file_id, "value_analysis", &a)?;
            println!("    {}", a.lines().next().unwrap_or(""));
        }
        Err(e) => println!("failed: {e}"),
    }

    print!("  attribution... ");
    match classify_llm_attribution(&client, &content).await {
        Ok(a) => {
            println!("{a}");
            store_llm_attribution(conn, file_id, &a)?;
        }
        Err(e) => println!("failed: {e}"),
    }

    print!("  security scan... ");
    match detect_secrets_llm(&client, &content).await {
        Ok(s) if s != "NONE" && !s.is_empty() => {
            println!("findings detected");
            store_agent_event(conn, file_id, "llm_security_scan", &s)?;
        }
        _ => println!("clean"),
    }

    let val: Option<f64> = conn
        .query_row(
            "SELECT book_value_usd FROM valuations WHERE file_id = ?1",
            params![file_id],
            |row| row.get(0),
        )
        .ok();
    let sec_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM security_findings WHERE file_id = ?1",
        params![file_id],
        |row| row.get(0),
    )?;

    if let Some(v) = val {
        print!("  evidence card... ");
        match generate_evidence_card(&client, &path_str, &content, sec_count as usize, v).await {
            Ok(card) => {
                println!("done");
                store_evidence_card(conn, file_id, &card)?;
            }
            Err(e) => println!("failed: {e}"),
        }
    }

    println!("  complete.");
    Ok(())
}

// ------------------------------------------------------------------
// DB helpers
// ------------------------------------------------------------------

fn find_pending_files(conn: &Connection, dir: &Path) -> Result<Vec<(i64, String)>> {
    let dir_str = format!("{}%", dir.to_string_lossy());
    let mut stmt = conn.prepare(
        r#"SELECT f.file_id, f.path
           FROM files f
           WHERE f.path LIKE ?1
             AND f.deleted_at IS NULL
             AND (
               SELECT COUNT(*) FROM file_events fe
               WHERE fe.file_id = f.file_id
                 AND fe.event_type LIKE 'agent_%'
                 AND fe.timestamp > f.modified_at
             ) = 0
           ORDER BY f.file_id
           LIMIT 50"#,
    )?;
    let rows = stmt.query_map(params![dir_str], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn store_agent_event(
    conn: &mut Connection,
    file_id: i64,
    event_type: &str,
    notes: &str,
) -> Result<()> {
    let now = chrono::Utc::now().timestamp() as f64;
    let path: String = conn.query_row(
        "SELECT path FROM files WHERE file_id = ?1",
        params![file_id],
        |row| row.get(0),
    )?;
    conn.execute(
        "INSERT INTO file_events (timestamp, file_id, path, event_type, notes, source, confidence) VALUES (?1, ?2, ?3, ?4, ?5, 'llm_agent', 0.7)",
        params![now, file_id, path, format!("agent_{}", event_type), notes],
    )?;
    Ok(())
}

fn store_llm_attribution(conn: &mut Connection, file_id: i64, attribution: &str) -> Result<()> {
    let confidence = if attribution.contains("human") {
        "high"
    } else if attribution.contains("mixed") {
        "medium"
    } else {
        "low"
    };
    conn.execute(
        "INSERT OR REPLACE INTO llm_attribution (file_id, source, confidence) VALUES (?1, ?2, ?3)",
        params![file_id, attribution, confidence],
    )?;
    Ok(())
}

fn store_evidence_card(conn: &mut Connection, file_id: i64, card_text: &str) -> Result<()> {
    conn.execute(
        r#"INSERT OR REPLACE INTO evidence_cards (
            file_id, summary, estimated_value, valuation_confidence, proof_notes
        ) VALUES (?1, ?2, ?3, ?4, ?5)"#,
        params![
            file_id,
            card_text.lines().next().unwrap_or(""),
            0.0f64,
            "llm",
            card_text,
        ],
    )?;
    Ok(())
}
