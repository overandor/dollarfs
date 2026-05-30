use anyhow::Result;
use regex::Regex;
use rusqlite::params;
use std::io::{BufRead, BufReader};

/// Maximum file size to scan for secrets (100 MB) to prevent memory exhaustion.
const MAX_SCAN_SIZE: u64 = 100 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct SecretPattern {
    pub name: &'static str,
    pub regex: Regex,
    pub severity: &'static str,
}

#[derive(Debug, Clone)]
pub struct SecretFinding {
    pub path: String,
    pub finding_type: String,
    pub line_number: usize,
    pub severity: String,
    pub preview: String,
    pub fingerprint: String,
}

pub fn build_patterns() -> Vec<SecretPattern> {
    vec![
        SecretPattern { name: "AWS Access Key ID", regex: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(), severity: "critical" },
        SecretPattern { name: "AWS Secret Access Key", regex: Regex::new(r#"(?i)aws[_\-]?secret[_\-]?access[_\-]?key\s*[=:]\s*[A-Za-z0-9/+=]{40}"#).unwrap(), severity: "critical" },
        SecretPattern { name: "GitHub Personal Access Token", regex: Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap(), severity: "critical" },
        SecretPattern { name: "GitHub OAuth Token", regex: Regex::new(r"gho_[A-Za-z0-9]{36}").unwrap(), severity: "critical" },
        SecretPattern { name: "Slack Token", regex: Regex::new(r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*").unwrap(), severity: "critical" },
        SecretPattern { name: "Private Key Block", regex: Regex::new(r"-----BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----").unwrap(), severity: "critical" },
        SecretPattern { name: "Generic API Key", regex: Regex::new(r#"(?i)(api[_\-]?key|apikey)\s*[=:]\s*[\"']?[A-Za-z0-9_\-]{16,}[\"']?"#).unwrap(), severity: "high" },
        SecretPattern { name: "Generic Secret", regex: Regex::new(r#"(?i)(secret|password|passwd|pwd)\s*[=:]\s*[\"']?[^\s\"']{8,}[\"']?"#).unwrap(), severity: "high" },
        SecretPattern { name: "Bearer Token", regex: Regex::new(r#"(?i)bearer\s+[A-Za-z0-9_\-\.=]{20,}"#).unwrap(), severity: "high" },
        SecretPattern { name: "JWT Token", regex: Regex::new(r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*").unwrap(), severity: "high" },
        SecretPattern { name: "URL with Password", regex: Regex::new(r"[a-zA-Z0-9]+://[^:]+:[^@]+@").unwrap(), severity: "high" },
        SecretPattern { name: "Ethereum Private Key", regex: Regex::new(r"0x[a-fA-F0-9]{64}").unwrap(), severity: "critical" },
        SecretPattern { name: "Bitcoin WIF", regex: Regex::new(r"[LK][1-9A-HJ-NP-Za-km-z]{51}").unwrap(), severity: "critical" },
        SecretPattern { name: "OpenAI API Key", regex: Regex::new(r"sk-[A-Za-z0-9]{20,48}").unwrap(), severity: "critical" },
        SecretPattern { name: "Stripe API Key", regex: Regex::new(r"sk_live_[A-Za-z0-9]{24,}").unwrap(), severity: "critical" },
        SecretPattern { name: "Google API Key", regex: Regex::new(r"AIza[0-9A-Za-z_\-]{35}").unwrap(), severity: "high" },
        SecretPattern { name: "Discord Bot Token", regex: Regex::new(r"[MN][A-Za-z\d]{23}\.[\w-]{6}\.[\w-]{27}").unwrap(), severity: "high" },
        SecretPattern { name: "SSH Key (DSA)", regex: Regex::new(r"-----BEGIN DSA PRIVATE KEY-----").unwrap(), severity: "critical" },
        SecretPattern { name: "SSH Key (EC)", regex: Regex::new(r"-----BEGIN EC PRIVATE KEY-----").unwrap(), severity: "critical" },
        SecretPattern { name: "Database Connection String", regex: Regex::new(r#"(?i)(postgres|mysql|mongodb|redis)://[^:]+:[^@]+@"#).unwrap(), severity: "critical" },
    ]
}

fn fingerprint_secret(text: &str) -> String {
    format!("blake3:{}", blake3::hash(text.as_bytes()).to_hex())
}

pub fn scan_text(content: &str, patterns: &[SecretPattern]) -> Vec<(String, usize, String, String, String)> {
    let mut findings = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        for pat in patterns {
            if let Some(m) = pat.regex.find(line) {
                let raw = m.as_str().to_string();
                findings.push((
                    pat.name.to_string(),
                    line_no + 1,
                    pat.severity.to_string(),
                    redact_preview(&raw),
                    fingerprint_secret(&raw),
                ));
            }
        }
    }
    findings
}

pub fn redact_preview(text: &str) -> String {
    if text.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}****{}", &text[..4], &text[text.len()-4..])
    }
}

pub fn scan_file(
    conn: &rusqlite::Connection,
    file_id: i64,
    path: &str,
    patterns: &[SecretPattern],
) -> Result<Vec<SecretFinding>> {
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return Ok(Vec::new()),
    };
    if metadata.len() > MAX_SCAN_SIZE {
        eprintln!("warn: security scan skipped {} ({} bytes > max {})", path, metadata.len(), MAX_SCAN_SIZE);
        return Ok(Vec::new());
    }

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(Vec::new()),
    };
    let reader = BufReader::new(file);
    let mut raw_findings = Vec::new();
    for (line_no, line) in reader.lines().flatten().enumerate() {
        for pat in patterns {
            if let Some(m) = pat.regex.find(&line) {
                let raw = m.as_str().to_string();
                raw_findings.push((
                    pat.name.to_string(),
                    line_no + 1,
                    pat.severity.to_string(),
                    redact_preview(&raw),
                    fingerprint_secret(&raw),
                ));
            }
        }
    }
    let mut findings = Vec::new();

    for (finding_type, line_number, severity, preview, fingerprint) in raw_findings {
        conn.execute(
            "INSERT INTO security_findings (file_id, finding_type, line_number, match_text, severity) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![file_id, &finding_type, line_number as i64, &fingerprint, &severity],
        )?;
        findings.push(SecretFinding {
            path: path.to_string(),
            finding_type,
            line_number,
            severity,
            preview,
            fingerprint,
        });
    }

    if !findings.is_empty() {
        conn.execute(
            "INSERT INTO file_events (file_id, path, event_type, notes) VALUES (?1, ?2, 'file_flagged_secret', ?3)",
            params![file_id, path, format!("{} secret patterns detected; raw values not stored", findings.len())],
        )?;
    }

    Ok(findings)
}

pub fn scan_directory(
    conn: &mut rusqlite::Connection,
    dir: &str,
) -> Result<Vec<SecretFinding>> {
    let patterns = build_patterns();
    let mut stmt = conn.prepare(
        "SELECT file_id, path FROM files WHERE path LIKE ?1 AND deleted_at IS NULL",
    )?;
    let rows = stmt.query_map(params![format!("{}%", dir)], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut all_findings: Vec<SecretFinding> = Vec::new();
    for row in rows {
        let (file_id, path) = row?;
        match scan_file(conn, file_id, &path, &patterns) {
            Ok(findings) => all_findings.extend(findings),
            Err(e) => eprintln!("warn: security scan failed for {}: {}", path, e),
        }
    }

    Ok(all_findings)
}

fn walk_dir(dir: &std::path::Path, cb: &mut dyn FnMut(&std::path::Path)) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, cb);
            } else if path.is_file() {
                cb(&path);
            }
        }
    }
}

pub fn scan_directory_raw(dir: &std::path::Path) -> Result<Vec<SecretFinding>> {
    let patterns = build_patterns();
    let mut all_findings: Vec<SecretFinding> = Vec::new();

    walk_dir(dir, &mut |path| {
        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.len() > MAX_SCAN_SIZE {
                return;
            }
        }
        if let Ok(file) = std::fs::File::open(path) {
            let reader = BufReader::new(file);
            for (line_no, line) in reader.lines().flatten().enumerate() {
                for pat in &patterns {
                    if let Some(m) = pat.regex.find(&line) {
                        let raw = m.as_str().to_string();
                        all_findings.push(SecretFinding {
                            path: path.to_string_lossy().to_string(),
                            finding_type: pat.name.to_string(),
                            line_number: line_no + 1,
                            severity: pat.severity.to_string(),
                            preview: redact_preview(&raw),
                            fingerprint: fingerprint_secret(&raw),
                        });
                    }
                }
            }
        }
    });

    Ok(all_findings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redact_preview_short() {
        assert_eq!(redact_preview("abc"), "****");
        assert_eq!(redact_preview("abcdefgh"), "****");
    }

    #[test]
    fn test_redact_preview_long() {
        assert_eq!(redact_preview("abcdefghijkl"), "abcd****ijkl");
    }

    #[test]
    fn test_detects_aws_key_without_exposing_raw_secret() {
        let patterns = build_patterns();
        let findings = scan_text("AWS=AKIA1234567890ABCDEF", &patterns);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].0, "AWS Access Key ID");
        assert!(findings[0].3.contains("****"));
        assert!(findings[0].4.starts_with("blake3:"));
        assert!(!findings[0].4.contains("AKIA"));
    }
}
