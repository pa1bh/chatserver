use dashmap::DashMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, error, info};
use uuid::Uuid;

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Default timeout for AI requests in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 30;
/// Default max tokens for AI responses
const DEFAULT_MAX_TOKENS: u32 = 1024;

#[derive(Clone)]
pub struct AiConfig {
    pub enabled: bool,
    pub api_key: String,
    pub model: String,
    pub rate_limit: u32,   // requests per minute per user
    pub timeout_secs: u64, // timeout for API requests
    pub max_tokens: u32,   // max tokens in AI response
}

impl AiConfig {
    pub fn from_env() -> Self {
        let enabled = std::env::var("AI_ENABLED")
            .map(|v| v.to_lowercase() == "true")
            .unwrap_or(false);

        let api_key = std::env::var("OPENROUTER_API_KEY").unwrap_or_default();

        let model = std::env::var("AI_MODEL").unwrap_or_else(|_| "openai/gpt-4o".to_string());

        let rate_limit = std::env::var("AI_RATE_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        let timeout_secs = std::env::var("AI_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);

        let max_tokens = std::env::var("AI_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(DEFAULT_MAX_TOKENS);

        if enabled && api_key.is_empty() {
            error!("AI_ENABLED=true but OPENROUTER_API_KEY is not set");
        }

        info!(
            enabled,
            model,
            rate_limit,
            timeout_secs,
            max_tokens,
            has_api_key = !api_key.is_empty(),
            "AI configuration loaded"
        );

        Self {
            enabled,
            api_key,
            model,
            rate_limit,
            timeout_secs,
            max_tokens,
        }
    }
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct Usage {
    total_tokens: Option<u32>,
    cost: Option<f64>,
}

/// Result of an AI query including response content and stats
#[derive(Debug, Clone)]
pub struct AiResponse {
    pub content: String,
    pub response_ms: u64,
    pub tokens: Option<u32>,
    pub cost: Option<f64>,
}

struct RateLimitEntry {
    count: u32,
    window_start: Instant,
}

pub struct AiClient {
    config: AiConfig,
    http: Client,
    rate_limits: Arc<DashMap<Uuid, RateLimitEntry>>,
}

impl AiClient {
    pub fn new(config: AiConfig) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            http,
            rate_limits: Arc::new(DashMap::new()),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled && !self.config.api_key.is_empty()
    }

    fn check_rate_limit(&self, user_id: Uuid) -> Result<(), String> {
        let now = Instant::now();
        let window = Duration::from_secs(60);

        let mut entry = self.rate_limits.entry(user_id).or_insert(RateLimitEntry {
            count: 0,
            window_start: now,
        });

        // Reset window if expired
        if now.duration_since(entry.window_start) >= window {
            entry.count = 0;
            entry.window_start = now;
        }

        if entry.count >= self.config.rate_limit {
            let remaining = window
                .checked_sub(now.duration_since(entry.window_start))
                .unwrap_or(Duration::ZERO);
            return Err(format!(
                "Rate limit bereikt (max {}/min). Probeer over {} seconden.",
                self.config.rate_limit,
                remaining.as_secs()
            ));
        }

        entry.count += 1;
        Ok(())
    }

    pub async fn query(&self, user_id: Uuid, prompt: &str) -> Result<AiResponse, String> {
        if !self.is_enabled() {
            return Err("AI is niet geactiveerd op deze server.".to_string());
        }

        // Check rate limit
        self.check_rate_limit(user_id)?;

        // Validate prompt
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err("Geef een vraag op. Gebruik: /ai <vraag>".to_string());
        }
        if prompt.len() > 1000 {
            return Err("Vraag is te lang (max 1000 tekens).".to_string());
        }

        debug!(user_id = %user_id, prompt_len = prompt.len(), "Sending AI request");

        let request = ChatRequest {
            model: self.config.model.clone(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: self.config.max_tokens,
        };

        let start = Instant::now();

        let response = self
            .http
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                error!(?e, "OpenRouter request failed");
                if e.is_timeout() {
                    format!(
                        "AI request timed out after {} seconds.",
                        self.config.timeout_secs
                    )
                } else {
                    "AI service tijdelijk niet beschikbaar.".to_string()
                }
            })?;

        let response_ms = start.elapsed().as_millis() as u64;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!(%status, %body, "OpenRouter error response");
            return Err(format!("AI service error: {}", status));
        }

        let chat_response: ChatResponse = response.json().await.map_err(|e| {
            error!(?e, "Failed to parse OpenRouter response");
            "Kon AI antwoord niet verwerken.".to_string()
        })?;

        let content = chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .unwrap_or_else(|| "Geen antwoord ontvangen.".to_string());

        let tokens = chat_response.usage.as_ref().and_then(|u| u.total_tokens);
        let cost = chat_response.usage.as_ref().and_then(|u| u.cost);

        debug!(
            response_len = content.len(),
            response_ms,
            ?tokens,
            ?cost,
            "AI response received"
        );

        Ok(AiResponse {
            content,
            response_ms,
            tokens,
            cost,
        })
    }
}
