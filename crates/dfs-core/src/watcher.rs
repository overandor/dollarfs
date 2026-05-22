use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

use crate::scanner::{index_file, is_excluded, DEFAULT_EXCLUDED};

pub fn create_watcher() -> Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
    let (tx, rx) = channel();
    let watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            let _ = tx.send(res);
        },
        Config::default().with_poll_interval(Duration::from_secs(2)),
    )?;
    Ok((watcher, rx))
}

pub fn watch_directory(watcher: &mut RecommendedWatcher, path: &Path) -> Result<()> {
    watcher.watch(path, RecursiveMode::Recursive)?;
    Ok(())
}

pub fn process_events(
    rx: &Receiver<notify::Result<Event>>,
    conn: &mut Connection,
    timeout: Duration,
) -> Result<usize> {
    let mut processed = 0usize;
    let deadline = std::time::Instant::now() + timeout;

    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(event)) => {
                for path in &event.paths {
                    if is_excluded(path, DEFAULT_EXCLUDED) {
                        continue;
                    }
                    match event.kind {
                        notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                            if path.is_file() {
                                let tx = conn.transaction()?;
                                match index_file(&tx, path) {
                                    Ok(_) => {
                                        tx.commit()?;
                                        processed += 1;
                                    }
                                    Err(e) => {
                                        let _ = tx.rollback();
                                        tracing::warn!("index failed for {}: {}", path.display(), e);
                                    }
                                }
                            }
                        }
                        notify::EventKind::Remove(_) => {
                            let path_str = path.to_string_lossy().to_string();
                            let now = chrono::Utc::now().timestamp() as f64;
                            conn.execute(
                                "UPDATE files SET deleted_at = ?1 WHERE path = ?2",
                                params![now, path_str],
                            )?;
                            processed += 1;
                        }
                        _ => {}
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::warn!("watch error: {}", e);
            }
            Err(_) => {
                // timeout, no more events in this batch
            }
        }
    }

    Ok(processed)
}

/// Run the watcher loop indefinitely. Callers should set up Ctrl-C handling.
pub fn run_watch_loop(
    paths: &[&Path],
    conn: &mut Connection,
) -> Result<()> {
    let (mut watcher, rx) = create_watcher()?;
    for path in paths {
        if path.exists() {
            watch_directory(&mut watcher, path)?;
            println!("  watching: {}", path.display());
        } else {
            println!("  skip (not found): {}", path.display());
        }
    }

    println!("lfv WATCH — monitoring file changes. Press Ctrl-C to stop.");
    loop {
        match process_events(&rx, conn, Duration::from_secs(5)) {
            Ok(n) => {
                if n > 0 {
                    println!("  {} file(s) updated", n);
                }
            }
            Err(e) => {
                eprintln!("watch loop error: {}", e);
                break;
            }
        }
    }
    Ok(())
}
