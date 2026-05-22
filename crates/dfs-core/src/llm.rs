use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for LLM integration — persisted in settings table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
    pub timeout_seconds: u64,
    pub max_tokens: u32,
    pub temperature: f32,
    pub enabled: bool,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434/v1/chat/completions".to_string(), // Ollama default
            model: "llama3.1".to_string(),
            api_key: None,
            timeout_seconds: 120,
            max_tokens: 2048,
            temperature: 0.3,
            enabled: false,
        }
    }
}

impl LlmConfig {
    pub fn load_from_db(conn: &rusqlite::Connection) -> Result<Self> {
        let mut stmt = conn.prepare("SELECT key, value FROM settings WHERE key LIKE 'llm_%'")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut cfg = Self::default();
        for row in rows {
            let (k, v) = row?;
            match k.as_str() {
                "llm_endpoint" => cfg.endpoint = v,
                "llm_model" => cfg.model = v,
                "llm_api_key" => cfg.api_key = Some(v),
                "llm_timeout" => cfg.timeout_seconds = v.parse().unwrap_or(120),
                "llm_max_tokens" => cfg.max_tokens = v.parse().unwrap_or(2048),
                "llm_temperature" => cfg.temperature = v.parse().unwrap_or(0.3),
                "llm_enabled" => cfg.enabled = v.parse().unwrap_or(false),
                _ => {}
            }
        }
        Ok(cfg)
    }

    pub fn save_to_db(&self, conn: &rusqlite::Connection) -> Result<()> {
        let pairs = [
            ("llm_endpoint", self.endpoint.as_str()),
            ("llm_model", self.model.as_str()),
            ("llm_api_key", self.api_key.as_deref().unwrap_or("")),
            ("llm_timeout", &self.timeout_seconds.to_string()),
            ("llm_max_tokens", &self.max_tokens.to_string()),
            ("llm_temperature", &self.temperature.to_string()),
            ("llm_enabled", &self.enabled.to_string()),
        ];
        for (k, v) in &pairs {
            conn.execute(
                "INSERT OR REPLACE INTO settings(key, value) VALUES (?1, ?2)",
                rusqlite::params![k, v],
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

#[derive(Debug, Deserialize)]
struct Message {
    content: String,
}

/// Low-level LLM client. Works with any OpenAI-compatible endpoint (Ollama, LM Studio, OpenAI, etc.)
pub struct LlmClient {
    http: reqwest::Client,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .context("failed to build HTTP client")?;
        Ok(Self { http, config })
    }

    pub async fn chat(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        if !self.config.enabled {
            anyhow::bail!("LLM is disabled. Enable with `lfv llm-config --enable`.");
        }

        let req_body = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: system_prompt.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: user_prompt.to_string(),
                },
            ],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
        };

        let mut builder = self
            .http
            .post(&self.config.endpoint)
            .header("Content-Type", "application/json");

        if let Some(ref key) = self.config.api_key {
            builder = builder.header("Authorization", format!("Bearer {}", key));
        }

        let resp = builder
            .json(&req_body)
            .send()
            .await
            .context("LLM HTTP request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("LLM returned {}: {}", status, body);
        }

        let data: ChatCompletionResponse = resp
            .json()
            .await
            .context("failed to parse LLM JSON response")?;

        let content = data
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .unwrap_or_default()
            .trim()
            .to_string();

        Ok(content)
    }

    /// Ping the endpoint to verify connectivity.
    pub async fn ping(&self) -> Result<String> {
        if !self.config.enabled {
            anyhow::bail!("LLM disabled");
        }
        // Lightweight ping via a tiny chat request
        let resp = self
            .chat("You are a health-check assistant.", "Respond with exactly: OK")
            .await?;
        Ok(resp)
    }
}

/// High-level file analysis prompts.
pub async fn analyze_file_value(client: &LlmClient, path: &str, content: &str) -> Result<String> {
    let system = "You are a software asset valuation analyst. Analyze the provided file content and return a concise assessment of its economic value as a software asset. Consider: complexity, uniqueness, reusability, R&D investment, production-readiness. Respond in 2-4 sentences.";
    let user = format!("File: {}\n\nContent (first 4000 chars):\n{}", path, &content[..content.len().min(4000)]);
    client.chat(system, &user).await
}

pub async fn classify_llm_attribution(client: &LlmClient, content: &str) -> Result<String> {
    let system = "You are a code provenance classifier. Examine the text and classify whether it appears to be: 'human' (hand-crafted by a person), 'ai-generated' (mostly produced by an LLM with minimal editing), or 'mixed' (substantial AI output with human editing). Respond with exactly one word: human, ai-generated, or mixed. No explanation.";
    let user = format!("Text sample:\n{}", &content[..content.len().min(2000)]);
    client.chat(system, &user).await
}

pub async fn generate_evidence_card(
    client: &LlmClient,
    path: &str,
    content: &str,
    security_findings: usize,
    value_usd: f64,
) -> Result<String> {
    let system = "You are a software asset evidence analyst. Produce a structured evidence card as plain text with these sections: Summary, R&D Qualification, Risks, Next Action. Keep each section to 1-2 sentences.";
    let user = format!(
        "File: {}\nEstimated value: ${:.2}\nSecurity findings: {}\n\nContent (first 3000 chars):\n{}",
        path, value_usd, security_findings, &content[..content.len().min(3000)]
    );
    client.chat(system, &user).await
}

pub async fn detect_secrets_llm(client: &LlmClient, content: &str) -> Result<String> {
    let system = "You are a security auditor. Scan the provided text for any secrets, credentials, API keys, tokens, or private keys that may have been missed by regex scanners. List each finding as: LINE: TYPE - brief description. If none, respond: NONE.";
    let user = format!("Text:\n{}", &content[..content.len().min(4000)]);
    client.chat(system, &user).await
}
