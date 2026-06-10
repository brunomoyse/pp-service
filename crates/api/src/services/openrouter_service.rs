//! OpenRouter integration for AI-assisted roster import.
//!
//! Takes a parsed spreadsheet (`headers` + `rows`) and asks an LLM to map the
//! arbitrary columns into one clean player display name per row — combining
//! first/last name columns, fixing casing, ignoring noise columns (email,
//! phone, totals). The model is told to NEVER invent people and to return one
//! entry per input row, keyed by row index, as strict JSON.
//!
//! Configured entirely from the environment and optional: when
//! `OPENROUTER_API_KEY` is absent the service is `None` and the import flow
//! falls back to manual column mapping in the client.

use serde::Deserialize;
use serde_json::json;
use thiserror::Error;
use tracing::warn;

/// Cap rows per LLM call to keep request/response within token + body limits.
/// Larger imports are chunked across multiple calls.
const MAX_ROWS_PER_CALL: usize = 200;

#[derive(Debug, Error)]
pub enum OpenRouterError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("API error (status {status}): {body}")]
    ApiError { status: u16, body: String },
    #[error("Could not parse model response: {0}")]
    Parse(String),
}

#[derive(Clone)]
pub struct OpenRouterConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
}

impl OpenRouterConfig {
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY").ok()?;
        Some(Self {
            api_key,
            model: std::env::var("OPENROUTER_MODEL")
                .unwrap_or_else(|_| "deepseek/deepseek-chat-v4-flash".to_string()),
            base_url: std::env::var("OPENROUTER_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
        })
    }
}

/// One normalized player produced by the model.
#[derive(Debug, Clone)]
pub struct NormalizedPlayer {
    pub source_row_index: usize,
    pub display_name: String,
}

#[derive(Clone)]
pub struct OpenRouterService {
    config: OpenRouterConfig,
    client: reqwest::Client,
}

// ── Response shapes ─────────────────────────────────────────────────

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

/// The JSON object the model is instructed to return.
#[derive(Deserialize)]
struct ModelOutput {
    players: Vec<ModelPlayer>,
}

#[derive(Deserialize)]
struct ModelPlayer {
    #[serde(rename = "row")]
    source_row_index: usize,
    #[serde(rename = "name")]
    display_name: String,
}

const SYSTEM_PROMPT: &str = "You normalize a club's player roster imported from a spreadsheet. \
You receive column headers and data rows (rows are arrays aligned to the headers). \
For EACH input row, output exactly one player with a clean full display name: \
combine first/last name columns, fix capitalization (e.g. \"jean DUPONT\" -> \"Jean Dupont\"), \
trim whitespace, and ignore non-name columns such as email, phone, points, or totals. \
NEVER invent, merge, drop, or reorder people: output one entry per input row, in order. \
If a row has no usable name, use an empty string for that row's name. \
Respond with STRICT JSON only, no prose, in the shape: \
{\"players\":[{\"row\":<0-based input row index>,\"name\":\"<clean name>\"}]}";

impl OpenRouterService {
    pub fn new(config: OpenRouterConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Normalize a parsed spreadsheet into clean display names, one per row.
    /// Rows are chunked to stay within model limits; row indices in the result
    /// are absolute (relative to the full `rows` input).
    pub async fn normalize_roster(
        &self,
        headers: &[String],
        rows: &[Vec<String>],
    ) -> Result<Vec<NormalizedPlayer>, OpenRouterError> {
        let mut out = Vec::with_capacity(rows.len());
        for (chunk_idx, chunk) in rows.chunks(MAX_ROWS_PER_CALL).enumerate() {
            let offset = chunk_idx * MAX_ROWS_PER_CALL;
            let normalized = self.normalize_chunk(headers, chunk).await?;
            for p in normalized {
                out.push(NormalizedPlayer {
                    source_row_index: offset + p.source_row_index,
                    display_name: p.display_name,
                });
            }
        }
        Ok(out)
    }

    async fn normalize_chunk(
        &self,
        headers: &[String],
        rows: &[Vec<String>],
    ) -> Result<Vec<NormalizedPlayer>, OpenRouterError> {
        let user_payload = json!({ "headers": headers, "rows": rows }).to_string();

        let body = json!({
            "model": self.config.model,
            "response_format": { "type": "json_object" },
            "temperature": 0,
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user", "content": user_payload },
            ],
        });

        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .bearer_auth(&self.config.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| OpenRouterError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(OpenRouterError::ApiError { status, body });
        }

        let parsed: ChatResponse = response
            .json()
            .await
            .map_err(|e| OpenRouterError::Parse(e.to_string()))?;

        let content = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content)
            .ok_or_else(|| OpenRouterError::Parse("empty choices".to_string()))?;

        let output: ModelOutput = serde_json::from_str(&content)
            .map_err(|e| OpenRouterError::Parse(format!("{e}: {content}")))?;

        // Keep only rows the model returned an index for and a non-empty name.
        let players = output
            .players
            .into_iter()
            .filter(|p| p.source_row_index < rows.len())
            .filter_map(|p| {
                let name = p.display_name.trim().to_string();
                if name.is_empty() {
                    None
                } else {
                    Some(NormalizedPlayer {
                        source_row_index: p.source_row_index,
                        display_name: name,
                    })
                }
            })
            .collect::<Vec<_>>();

        if players.is_empty() {
            warn!("OpenRouter returned no usable players for a roster import chunk");
        }
        Ok(players)
    }
}
