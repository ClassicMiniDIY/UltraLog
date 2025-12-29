//! LLM integration module for ECU log analysis.
//!
//! This module provides optional integration with OpenAI-compatible LLM endpoints
//! for AI-assisted ECU log triage and analysis.
//!
//! ## Features
//!
//! - Supports any OpenAI-compatible API (OpenAI, Ollama, LM Studio, vLLM, etc.)
//! - Vision mode for chart screenshot analysis
//! - Integration with local anomaly detection results
//! - Background request handling to keep UI responsive
//!
//! ## Privacy & Safety
//!
//! - Entirely optional and disabled by default
//! - User must explicitly configure and enable
//! - Clear warnings about data being sent externally
//! - API keys stored locally only

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::anomaly::AnomalyResults;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for LLM integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    /// Whether LLM integration is enabled
    pub enabled: bool,
    /// API endpoint URL (e.g., "https://api.openai.com/v1")
    pub endpoint_url: String,
    /// API key (optional for local endpoints)
    pub api_key: Option<String>,
    /// Model name (e.g., "gpt-4o-mini", "llama3.2")
    pub model: String,
    /// Maximum response tokens
    pub max_tokens: u32,
    /// Temperature (0.0-1.0)
    pub temperature: f32,
    /// Whether to include chart screenshots (requires vision model)
    pub vision_enabled: bool,
    /// Vision detail level: "low", "high", or "auto"
    pub vision_detail: String,
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint_url: String::new(),
            api_key: None,
            model: String::new(),
            max_tokens: 1000,
            temperature: 0.3,
            vision_enabled: true,
            vision_detail: "high".to_string(),
        }
    }
}

impl LlmConfig {
    /// Preset for OpenAI
    pub fn openai_preset() -> Self {
        Self {
            enabled: false,
            endpoint_url: "https://api.openai.com/v1".to_string(),
            api_key: None,
            model: "gpt-4o-mini".to_string(),
            max_tokens: 1000,
            temperature: 0.3,
            vision_enabled: true,
            vision_detail: "high".to_string(),
        }
    }

    /// Preset for Ollama (local)
    pub fn ollama_preset() -> Self {
        Self {
            enabled: false,
            endpoint_url: "http://localhost:11434/v1".to_string(),
            api_key: None,
            model: "llama3.2".to_string(),
            max_tokens: 1000,
            temperature: 0.3,
            vision_enabled: false, // Most Ollama models don't support vision
            vision_detail: "low".to_string(),
        }
    }

    /// Preset for LM Studio (local)
    pub fn lm_studio_preset() -> Self {
        Self {
            enabled: false,
            endpoint_url: "http://localhost:1234/v1".to_string(),
            api_key: None,
            model: "local-model".to_string(),
            max_tokens: 1000,
            temperature: 0.3,
            vision_enabled: false,
            vision_detail: "low".to_string(),
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.endpoint_url.is_empty() {
            return Err("Endpoint URL is required".to_string());
        }
        if self.model.is_empty() {
            return Err("Model name is required".to_string());
        }
        if self.max_tokens == 0 {
            return Err("Max tokens must be greater than 0".to_string());
        }
        if self.temperature < 0.0 || self.temperature > 2.0 {
            return Err("Temperature must be between 0.0 and 2.0".to_string());
        }
        Ok(())
    }
}

// ============================================================================
// API Types (OpenAI-compatible)
// ============================================================================

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: u32,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: MessageContent,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Serialize)]
struct ImageUrl {
    url: String,
    detail: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    #[allow(dead_code)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[allow(dead_code)]
    prompt_tokens: u32,
    #[allow(dead_code)]
    completion_tokens: u32,
    #[allow(dead_code)]
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct ErrorResponse {
    error: ApiError,
}

#[derive(Debug, Deserialize)]
struct ApiError {
    message: String,
}

// ============================================================================
// Analysis Context
// ============================================================================

/// Context for LLM analysis request
#[derive(Debug, Clone)]
pub struct AnalysisContext {
    /// Time range being analyzed (start, end) in seconds
    pub time_range: (f64, f64),
    /// Channel information: (name, min, max, current_value)
    pub channels: Vec<ChannelSummary>,
    /// Anomaly detection results (if available)
    pub anomalies: Option<AnomalyResults>,
    /// Chart screenshot as PNG bytes (if vision enabled)
    pub chart_image: Option<Vec<u8>>,
    /// User's question or analysis request
    pub user_prompt: String,
}

/// Summary of a channel for LLM context
#[derive(Debug, Clone)]
pub struct ChannelSummary {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
    pub current: f64,
    pub unit: Option<String>,
}

impl AnalysisContext {
    /// Generate the text portion of the prompt
    pub fn to_prompt_text(&self) -> String {
        let mut prompt = String::new();

        // Header
        prompt.push_str("You are an expert automotive ECU tuner analyzing engine log data.\n");
        prompt.push_str("Be concise and focus on actionable insights. Identify any issues that could indicate:\n");
        prompt.push_str("- Engine tuning problems (lean/rich conditions, knock, timing issues)\n");
        prompt.push_str("- Sensor failures or anomalies\n");
        prompt.push_str("- Mechanical issues (boost leaks, fuel system problems)\n\n");

        // Time range
        prompt.push_str(&format!(
            "TIME RANGE: {:.2}s to {:.2}s ({:.1}s duration)\n\n",
            self.time_range.0,
            self.time_range.1,
            self.time_range.1 - self.time_range.0
        ));

        // Channel data
        prompt.push_str("CHANNELS:\n");
        for ch in &self.channels {
            let unit_str = ch.unit.as_deref().unwrap_or("");
            prompt.push_str(&format!(
                "- {}: min={:.2}{} max={:.2}{} avg={:.2}{} current={:.2}{}\n",
                ch.name, ch.min, unit_str, ch.max, unit_str, ch.avg, unit_str, ch.current, unit_str
            ));
        }
        prompt.push('\n');

        // Anomaly results
        if let Some(ref anomalies) = self.anomalies {
            prompt.push_str(&anomalies.to_prompt_summary());
            prompt.push('\n');
        }

        // User's question
        prompt.push_str("USER REQUEST:\n");
        prompt.push_str(&self.user_prompt);

        prompt
    }
}

// ============================================================================
// LLM Client
// ============================================================================

/// Result of an LLM analysis request
#[derive(Debug, Clone)]
pub enum LlmResult {
    Success {
        response: String,
        tokens_used: Option<u32>,
    },
    Error(String),
}

/// State of an ongoing LLM request
#[derive(Debug, Clone, Default)]
pub enum LlmRequestState {
    #[default]
    Idle,
    Pending,
    Complete(LlmResult),
}

/// Handles communication with LLM endpoint
pub struct LlmClient {
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Self {
        Self { config }
    }

    /// Test connection to the LLM endpoint
    pub fn test_connection(&self) -> Result<String, String> {
        if let Err(e) = self.config.validate() {
            return Err(e);
        }

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content: MessageContent::Text("Say 'Connection successful!' and nothing else.".to_string()),
            }],
            max_tokens: 20,
            temperature: 0.0,
        };

        self.send_request(&request)
    }

    /// Send an analysis request
    pub fn analyze(&self, context: &AnalysisContext) -> Result<String, String> {
        if !self.config.enabled {
            return Err("LLM integration is not enabled".to_string());
        }

        if let Err(e) = self.config.validate() {
            return Err(e);
        }

        let content = if self.config.vision_enabled {
            if let Some(ref image_bytes) = context.chart_image {
                // Vision request with image
                let base64_image = BASE64.encode(image_bytes);
                MessageContent::Parts(vec![
                    ContentPart::Text {
                        text: context.to_prompt_text(),
                    },
                    ContentPart::ImageUrl {
                        image_url: ImageUrl {
                            url: format!("data:image/png;base64,{}", base64_image),
                            detail: self.config.vision_detail.clone(),
                        },
                    },
                ])
            } else {
                // Fallback to text-only if no image available
                MessageContent::Text(context.to_prompt_text())
            }
        } else {
            MessageContent::Text(context.to_prompt_text())
        };

        let request = ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![Message {
                role: "user".to_string(),
                content,
            }],
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
        };

        self.send_request(&request)
    }

    /// Send request to API
    fn send_request(&self, request: &ChatCompletionRequest) -> Result<String, String> {
        let url = format!("{}/chat/completions", self.config.endpoint_url.trim_end_matches('/'));

        let mut http_request = ureq::post(&url)
            .header("Content-Type", "application/json");

        if let Some(ref api_key) = self.config.api_key {
            if !api_key.is_empty() {
                http_request = http_request.header("Authorization", &format!("Bearer {}", api_key));
            }
        }

        let response = http_request
            .send_json(request)
            .map_err(|e| {
                // Try to extract error message from response
                if let ureq::Error::StatusCode(code) = &e {
                    format!("HTTP {}: {}", code, e)
                } else {
                    format!("Request failed: {}", e)
                }
            })?;

        let response_text = response
            .into_body()
            .read_to_string()
            .map_err(|e| format!("Failed to read response: {}", e))?;

        // Try to parse as success response
        if let Ok(parsed) = serde_json::from_str::<ChatCompletionResponse>(&response_text) {
            if let Some(choice) = parsed.choices.first() {
                return Ok(choice.message.content.clone());
            }
            return Err("No response content from LLM".to_string());
        }

        // Try to parse as error response
        if let Ok(error) = serde_json::from_str::<ErrorResponse>(&response_text) {
            return Err(error.error.message);
        }

        Err(format!("Unexpected response: {}", response_text))
    }
}

// ============================================================================
// Background Request Handler
// ============================================================================

/// Message to send to the LLM worker thread
pub enum LlmRequest {
    TestConnection(LlmConfig),
    Analyze(LlmConfig, AnalysisContext),
}

/// Response from the LLM worker thread
pub enum LlmResponse {
    ConnectionTest(Result<String, String>),
    Analysis(LlmResult),
}

/// Spawn a background thread for LLM requests
pub fn spawn_llm_worker() -> (Sender<LlmRequest>, Receiver<LlmResponse>) {
    let (request_tx, request_rx) = channel::<LlmRequest>();
    let (response_tx, response_rx) = channel::<LlmResponse>();

    thread::spawn(move || {
        while let Ok(request) = request_rx.recv() {
            let response = match request {
                LlmRequest::TestConnection(config) => {
                    let client = LlmClient::new(config);
                    LlmResponse::ConnectionTest(client.test_connection())
                }
                LlmRequest::Analyze(config, context) => {
                    let client = LlmClient::new(config);
                    let result = match client.analyze(&context) {
                        Ok(response) => LlmResult::Success {
                            response,
                            tokens_used: None, // Could parse from API response if needed
                        },
                        Err(e) => LlmResult::Error(e),
                    };
                    LlmResponse::Analysis(result)
                }
            };

            if response_tx.send(response).is_err() {
                break; // Main thread dropped the receiver
            }
        }
    });

    (request_tx, response_rx)
}

// ============================================================================
// Warning Messages
// ============================================================================

pub const PRIVACY_WARNING: &str =
    "This feature sends ECU log data (channel names, values, timestamps) to an external \
     server. Do not use with proprietary or confidential tuning data.";

pub const ACCURACY_WARNING: &str =
    "LLM responses are AI-generated and may contain errors. Always verify analysis with \
     proper diagnostic tools. Never rely solely on LLM output for safety-critical decisions.";

pub const COST_WARNING: &str =
    "API calls may incur costs depending on your provider. You are responsible for any \
     charges from your API usage.";

pub const API_KEY_WARNING: &str =
    "API keys are stored locally on your machine. They are not encrypted and should be \
     treated as sensitive credentials.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = LlmConfig::default();
        assert!(config.validate().is_err());

        config.endpoint_url = "https://api.openai.com/v1".to_string();
        assert!(config.validate().is_err()); // Still missing model

        config.model = "gpt-4o-mini".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_preset_validation() {
        assert!(LlmConfig::openai_preset().validate().is_ok());
        assert!(LlmConfig::ollama_preset().validate().is_ok());
        assert!(LlmConfig::lm_studio_preset().validate().is_ok());
    }

    #[test]
    fn test_analysis_context_prompt() {
        let context = AnalysisContext {
            time_range: (10.0, 20.0),
            channels: vec![
                ChannelSummary {
                    name: "AFR".to_string(),
                    min: 12.0,
                    max: 15.0,
                    avg: 14.0,
                    current: 14.5,
                    unit: Some(":1".to_string()),
                },
            ],
            anomalies: None,
            chart_image: None,
            user_prompt: "What do you see?".to_string(),
        };

        let prompt = context.to_prompt_text();
        assert!(prompt.contains("AFR"));
        assert!(prompt.contains("10.00s to 20.00s"));
        assert!(prompt.contains("What do you see?"));
    }

    #[test]
    fn test_config_validation_edge_cases() {
        // Test temperature bounds
        let mut config = LlmConfig::openai_preset();

        config.temperature = -0.1;
        assert!(config.validate().is_err());

        config.temperature = 2.1;
        assert!(config.validate().is_err());

        config.temperature = 0.0;
        assert!(config.validate().is_ok());

        config.temperature = 2.0;
        assert!(config.validate().is_ok());

        // Test max_tokens
        config.max_tokens = 0;
        assert!(config.validate().is_err());

        config.max_tokens = 1;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_analysis_context_with_anomalies() {
        use crate::anomaly::{Anomaly, AnomalyResults, AnomalySeverity, AnomalyType};

        let mut anomaly_results = AnomalyResults::default();
        anomaly_results.channels_analyzed = 2;
        anomaly_results.points_analyzed = 500;
        anomaly_results.critical_count = 1;
        anomaly_results.anomalies = vec![Anomaly {
            channel_name: "AFR".to_string(),
            data_index: 10,
            time: 5.0,
            value: 8.0,
            anomaly_type: AnomalyType::RangeViolation {
                value: 8.0,
                expected_min: 10.0,
                expected_max: 20.0,
            },
            severity: AnomalySeverity::Critical,
        }];

        let context = AnalysisContext {
            time_range: (0.0, 10.0),
            channels: vec![ChannelSummary {
                name: "AFR".to_string(),
                min: 8.0,
                max: 15.0,
                avg: 14.0,
                current: 14.5,
                unit: None,
            }],
            anomalies: Some(anomaly_results),
            chart_image: None,
            user_prompt: "Analyze".to_string(),
        };

        let prompt = context.to_prompt_text();
        assert!(prompt.contains("ANOMALY DETECTION RESULTS"));
        assert!(prompt.contains("2 channels"));
        assert!(prompt.contains("1 critical"));
    }

    #[test]
    fn test_channel_summary_formatting() {
        let context = AnalysisContext {
            time_range: (0.0, 60.0),
            channels: vec![
                ChannelSummary {
                    name: "Boost Pressure".to_string(),
                    min: 0.0,
                    max: 22.5,
                    avg: 15.0,
                    current: 18.0,
                    unit: Some(" psi".to_string()),
                },
                ChannelSummary {
                    name: "RPM".to_string(),
                    min: 800.0,
                    max: 7200.0,
                    avg: 4500.0,
                    current: 6000.0,
                    unit: None,
                },
            ],
            anomalies: None,
            chart_image: None,
            user_prompt: "Check boost".to_string(),
        };

        let prompt = context.to_prompt_text();
        assert!(prompt.contains("Boost Pressure"));
        assert!(prompt.contains("RPM"));
        assert!(prompt.contains("22.50"));
        assert!(prompt.contains("psi"));
    }

    #[test]
    fn test_severity_strings() {
        use crate::anomaly::AnomalySeverity;

        assert_eq!(AnomalySeverity::Info.as_str(), "info");
        assert_eq!(AnomalySeverity::Warning.as_str(), "warning");
        assert_eq!(AnomalySeverity::Critical.as_str(), "critical");

        // Emoji tests
        assert!(!AnomalySeverity::Info.emoji().is_empty());
        assert!(!AnomalySeverity::Warning.emoji().is_empty());
        assert!(!AnomalySeverity::Critical.emoji().is_empty());
    }

    #[test]
    fn test_warning_constants_not_empty() {
        assert!(!PRIVACY_WARNING.is_empty());
        assert!(!ACCURACY_WARNING.is_empty());
        assert!(!COST_WARNING.is_empty());
        assert!(!API_KEY_WARNING.is_empty());
    }
}
