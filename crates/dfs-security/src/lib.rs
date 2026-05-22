use anyhow::Result;
use regex::Regex;
use rusqlite::params;

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
}

pub fn build_patterns() -> Vec<SecretPattern> {
    vec![
        SecretPattern {
            name: "AWS Access Key ID",
            regex: Regex::new(r"AKIA[0-9A-Z]{16}").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "AWS Secret Access Key",
            regex: Regex::new(r#"(?i)aws[_\-]?secret[_\-]?access[_\-]?key\s*[=:]\s*[A-Za-z0-9/+=]{40}"#).unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "GitHub Personal Access Token",
            regex: Regex::new(r"ghp_[A-Za-z0-9]{36}").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "GitHub OAuth Token",
            regex: Regex::new(r"gho_[A-Za-z0-9]{36}").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "Slack Token",
            regex: Regex::new(r"xox[baprs]-[0-9]{10,13}-[0-9]{10,13}[a-zA-Z0-9-]*").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "Private Key Block",
            regex: Regex::new(r"-----BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY-----").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "Generic API Key",
            regex: Regex::new(r#"(?i)(api[_\-]?key|apikey)\s*[=:]\s*[\"']?[A-Za-z0-9_\-]{16,}[\"']?"#).unwrap(),
            severity: "high",
        },
        SecretPattern {
            name: "Generic Secret",
            regex: Regex::new(r#"(?i)(secret|password|passwd|pwd)\s*[=:]\s*[\"']?[^\s\"']{8,}[\"']?"#).unwrap(),
            severity: "high",
        },
        SecretPattern {
            name: "Bearer Token",
            regex: Regex::new(r#"(?i)bearer\s+[A-Za-z0-9_\-\.=]{20,}"#).unwrap(),
            severity: "high",
        },
        SecretPattern {
            name: "JWT Token",
            regex: Regex::new(r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*").unwrap(),
            severity: "high",
        },
        SecretPattern {
            name: "URL with Password",
            regex: Regex::new(r"[a-zA-Z0-9]+://[^:]+:[^@]+@").unwrap(),
            severity: "high",
        },
        SecretPattern {
            name: "Ethereum Private Key",
            regex: Regex::new(r"0x[a-fA-F0-9]{64}").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "Bitcoin WIF",
            regex: Regex::new(r"[LK][1-9A-HJ-NP-Za-km-z]{51}").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "OpenAI API Key",
            regex: Regex::new(r"sk-[A-Za-z0-9]{20,48}").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "Stripe API Key",
            regex: Regex::new(r"sk_live_[A-Za-z0-9]{24,}").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "Google API Key",
            regex: Regex::new(r"AIza[0-9A-Za-z_\-]{35}").unwrap(),
            severity: "high",
        },
        SecretPattern {
            name: "Discord Bot Token",
            regex: Regex::new(r"[MN][A-Za-z\d]{23}\.[\w-]{6}\.[\w-]{27}").unwrap(),
            severity: "high",
        },
        SecretPattern {
            name: "SSH Key (DSA)",
            regex: Regex::new(r"-----BEGIN DSA PRIVATE KEY-----").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "SSH Key (EC)",
            regex: Regex::new(r"-----BEGIN EC PRIVATE KEY-----").unwrap(),
            severity: "critical",
        },
        SecretPattern {
            name: "Database Connection String",
            regex: Regex::new(r#"(?i)(postgres|mysql|mongodb|redis|sqlite)://[^:]+:[^@]+@"#).unwrap(),
            severity: "critical",
        },
    ]
}

pub fn scan_text(content: &str, patterns: &[SecretPattern]) -> Vec<(String, usize, String, String)> {
    let mut findings = Vec::new();
    for (line_no, line) in content.lines().enumerate() {
        for pat in patterns {
            if let Some(m) = pat.regex.find(line) {
                findings.push((
                    pat.name.to_string(),
                    line_no + 1,
                    pat.severity.to_string(),
                    m.as_str().to_string(),
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
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };

    let raw_findings = scan_text(&content, patterns);
    let mut findings = Vec::new();

    for (finding_type, line_number, severity, match_text) in raw_findings {
        let preview = redact_preview(&match_text);
        conn.execute(
            "INSERT INTO security_findings (file_id, finding_type, line_number, match_text, severity) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![file_id, &finding_type, line_number as i64, &match_text, &severity],
        )?;
        findings.push(SecretFinding {
            path: path.to_string(),
            finding_type,
            line_number,
            severity,
            preview,
        });
    }

    if !findings.is_empty() {
        conn.execute(
            "INSERT INTO file_events (file_id, path, event_type, notes) VALUES (?1, ?2, 'file_flagged_secret', ?3)",
            params![file_id, path, format!("{} secret patterns detected", findings.len())],
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
            Ok(findings) => {
                all_findings.extend(findings);
            }
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
        if let Ok(content) = std::fs::read_to_string(path) {
            let raw = scan_text(&content, &patterns);
            for (finding_type, line_number, severity, match_text) in raw {
                let preview = redact_preview(&match_text);
                all_findings.push(SecretFinding {
                    path: path.to_string_lossy().to_string(),
                    finding_type,
                    line_number,
                    severity,
                    preview,
                });
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
        let r = redact_preview("AKIAIOSFODNN7EXAMPLE");
        assert!(r.starts_with("AKIA"));
        assert!(r.ends_with("MPLE"));
        assert!(r.contains("****"));
    }

    #[test]
    fn test_scan_text_patterns() {
        let patterns = build_patterns();
        let content = "api_key = \"sk-live-1234567890abcdef\"\n\nAnother line\n";
        let findings = scan_text(content, &patterns);
        assert!(!findings.is_empty());
        let has_api_key = findings.iter().any(|(name, _, _, _)| name == "Generic API Key");
        assert!(has_api_key, "should detect Generic API Key");
    }

    #[test]
    fn test_scan_text_no_match() {
        let patterns = build_patterns();
        let content = "hello world\nthis is just plain text\n";
        let findings = scan_text(content, &patterns);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_scan_text_line_numbers() {
        let patterns = build_patterns();
        let content = "line1\nline2\npassword = \"secret123\"\n";
        let findings = scan_text(content, &patterns);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].1, 3); // line 3
    }
}
