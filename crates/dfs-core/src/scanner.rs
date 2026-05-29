use anyhow::Result;
use blake3::Hasher;
use rusqlite::{params, Connection};
use std::io::Read;
use std::path::Path;
use walkdir::WalkDir;

pub const DEFAULT_WATCHED: &[&str] = &[
    "Desktop",
    "Documents",
    "Developer",
    "Projects",
    "Code",
];

pub const DEFAULT_EXCLUDED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".DS_Store",
    "*.tmp",
    "*.swp",
    "*.pyc",
    "*.pyo",
    "*.log",
    "*.pid",
    "*.sock",
    ".env",
    ".env.local",
    ".env.*.local",
    ".npm",
    ".yarn",
    ".pnpm-store",
    ".next",
    ".nuxt",
    "coverage",
    ".idea",
    ".vscode",
    "Thumbs.db",
];

pub fn is_excluded(path: &Path, excluded: &[&str]) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
    for pat in excluded {
        if pat.starts_with("*") {
            if name.ends_with(&pat[1..]) {
                return true;
            }
        } else if name == *pat {
            return true;
        }
    }
    if let Some(parent) = path.parent() {
        let parent_name = parent.file_name().and_then(|s| s.to_str()).unwrap_or("");
        for pat in excluded {
            if !pat.starts_with("*") && parent_name == *pat {
                return true;
            }
        }
    }
    false
}

fn blake3_file(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Hasher::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn scan_path(conn: &mut Connection, root: &Path) -> Result<usize> {
    scan_path_incremental(conn, root, false)
}

pub fn scan_path_incremental(
    conn: &mut Connection,
    root: &Path,
    incremental: bool,
) -> Result<usize> {
    let tx = conn.transaction()?;
    let mut count = 0usize;

    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), DEFAULT_EXCLUDED))
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if is_excluded(path, DEFAULT_EXCLUDED) {
            continue;
        }

        if incremental {
            if let Ok(false) = needs_reindex(&tx, path) {
                continue;
            }
        }

        match index_file(&tx, path) {
            Ok(_) => count += 1,
            Err(e) => eprintln!("warn: failed to index {}: {}", path.display(), e),
        }
    }

    tx.commit()?;
    Ok(count)
}

fn needs_reindex(tx: &rusqlite::Transaction, path: &Path) -> Result<bool> {
    let path_str = path.to_string_lossy().to_string();
    let metadata = std::fs::metadata(path)?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64());

    let existing: Option<(f64, i64)> = tx.query_row(
        "SELECT modified_at, size FROM files WHERE path = ?1 AND deleted_at IS NULL",
        params![path_str],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).ok();

    if let Some((db_modified, db_size)) = existing {
        if let Some(m) = modified {
            if (m - db_modified).abs() < 1.0 && metadata.len() as i64 == db_size {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub fn detect_duplicates(conn: &mut Connection) -> Result<usize> {
    let mut stmt = conn.prepare(
        "SELECT file_id, content_hash FROM files WHERE deleted_at IS NULL ORDER BY content_hash"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    use std::collections::HashMap;
    let mut groups: HashMap<String, Vec<i64>> = HashMap::new();
    for row in rows {
        let (fid, hash) = row?;
        groups.entry(hash).or_default().push(fid);
    }
    drop(stmt);

    let mut group_id = 0i64;
    let mut updated = 0usize;
    let tx = conn.transaction()?;
    for (_hash, ids) in groups {
        if ids.len() > 1 {
            group_id += 1;
            for id in &ids {
                tx.execute(
                    "UPDATE files SET duplicate_group_id = ?1 WHERE file_id = ?2",
                    params![group_id, id],
                )?;
                updated += 1;
            }
        }
    }
    tx.commit()?;
    Ok(updated)
}

pub fn index_file(tx: &rusqlite::Transaction, path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(path)?;
    let size = metadata.len() as i64;
    let hash = blake3_file(path)?;

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());

    let mime = extension.as_ref().map(|ext| match ext.as_str() {
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "js" => "text/javascript",
        "ts" => "text/typescript",
        "tsx" => "text/typescript-jsx",
        "jsx" => "text/jsx",
        "go" => "text/x-go",
        "c" | "h" => "text/x-c",
        "cpp" | "hpp" | "cc" => "text/x-c++",
        "swift" => "text/x-swift",
        "md" => "text/markdown",
        "json" => "application/json",
        "toml" => "text/x-toml",
        "yaml" | "yml" => "text/x-yaml",
        "sql" => "text/x-sql",
        "sh" => "text/x-sh",
        "html" => "text/html",
        "css" => "text/css",
        "svg" => "image/svg+xml",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    });

    let created = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64());
    let modified = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64());

    let path_str = path.to_string_lossy().to_string();
    let now = chrono::Utc::now().timestamp() as f64;

    tx.execute(
        r#"INSERT INTO files (
            path, canonical_path, content_hash, size, extension, mime_guess,
            created_at, modified_at, indexed_at, last_seen_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(path) DO UPDATE SET
            content_hash = excluded.content_hash,
            size = excluded.size,
            modified_at = excluded.modified_at,
            last_seen_at = excluded.last_seen_at,
            deleted_at = NULL
        "#,
        params![
            path_str,
            path.canonicalize().ok().map(|p| p.to_string_lossy().to_string()),
            hash,
            size,
            extension,
            mime,
            created,
            modified,
            now,
            now,
        ],
    )?;

    let file_id: i64 = tx.query_row(
        "SELECT file_id FROM files WHERE path = ?1",
        params![path_str],
        |row| row.get(0),
    )?;

    tx.execute(
        "INSERT INTO file_events (file_id, path, event_type, after_hash, notes) VALUES (?1, ?2, 'file_indexed', ?3, 'Scanned by dfs')",
        params![file_id, path_str, hash],
    )?;

    Ok(())
}
