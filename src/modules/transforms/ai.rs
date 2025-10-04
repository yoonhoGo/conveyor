use anyhow::Result;
use async_trait::async_trait;
use handlebars::Handlebars;
use polars::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;

use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

#[derive(Debug, Clone)]
pub struct AiGenerateTransform {
    client: reqwest::Client,
}

impl Default for AiGenerateTransform {
    fn default() -> Self {
        Self::new()
    }
}

impl AiGenerateTransform {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum AiProvider {
    OpenAI,
    Anthropic,
    OpenRouter,
    Ollama,
}

// OpenAI API request/response types
#[derive(Debug, Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAIResponse {
    choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
}

impl<'de> Deserialize<'de> for OpenAIMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            role: String,
            content: String,
        }
        let helper = Helper::deserialize(deserializer)?;
        Ok(OpenAIMessage {
            role: helper.role,
            content: helper.content,
        })
    }
}

// Anthropic API request/response types
#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    text: String,
}

// Ollama API request/response types
#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

impl AiGenerateTransform {
    async fn call_openai(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<String> {
        let request = OpenAIRequest {
            model: model.to_string(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens,
            temperature,
        };

        let response = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("OpenAI API error ({}): {}", status, error_text);
        }

        let response_body: OpenAIResponse = response.json().await?;
        Ok(response_body
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))?
            .message
            .content
            .clone())
    }

    async fn call_anthropic(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<String> {
        let request = AnthropicRequest {
            model: model.to_string(),
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: max_tokens.unwrap_or(1024),
            temperature,
        };

        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Anthropic API error ({}): {}", status, error_text);
        }

        let response_body: AnthropicResponse = response.json().await?;
        Ok(response_body
            .content
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response from Anthropic"))?
            .text
            .clone())
    }

    async fn call_openrouter(
        &self,
        api_key: &str,
        model: &str,
        prompt: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<String> {
        // OpenRouter uses OpenAI-compatible API
        let request = OpenAIRequest {
            model: model.to_string(),
            messages: vec![OpenAIMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens,
            temperature,
        };

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("OpenRouter API error ({}): {}", status, error_text);
        }

        let response_body: OpenAIResponse = response.json().await?;
        Ok(response_body
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response from OpenRouter"))?
            .message
            .content
            .clone())
    }

    async fn call_ollama(
        &self,
        base_url: &str,
        model: &str,
        prompt: &str,
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<String> {
        let request = OllamaRequest {
            model: model.to_string(),
            prompt: prompt.to_string(),
            stream: false,
            options: Some(OllamaOptions {
                temperature,
                num_predict: max_tokens,
            }),
        };

        let url = format!("{}/api/generate", base_url.trim_end_matches('/'));
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Ollama API error ({}): {}", status, error_text);
        }

        let response_body: OllamaResponse = response.json().await?;
        Ok(response_body.response)
    }
}

#[async_trait]
impl Stage for AiGenerateTransform {
    fn name(&self) -> &str {
        "ai_generate"
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        let data = inputs
            .into_values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("AI transform requires input data"))?;

        // Parse provider
        let provider: AiProvider = config
            .get("provider")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("AI requires 'provider' configuration"))
            .and_then(|s| {
                serde_json::from_value(serde_json::Value::String(s.to_string()))
                    .map_err(|_| anyhow::anyhow!("Invalid provider: {}", s))
            })?;

        let model = config
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("AI requires 'model' configuration"))?;

        let prompt_template = config
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("AI requires 'prompt' configuration"))?;

        let output_column = config
            .get("output_column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("AI requires 'output_column' configuration"))?;

        let max_tokens = config
            .get("max_tokens")
            .and_then(|v| v.as_integer())
            .map(|i| i as u32);

        let temperature = config
            .get("temperature")
            .and_then(|v| v.as_float())
            .map(|f| f as f32);

        // Get API key from environment
        let api_key_env = config
            .get("api_key_env")
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| match provider {
                AiProvider::OpenAI => "OPENAI_API_KEY",
                AiProvider::Anthropic => "ANTHROPIC_API_KEY",
                AiProvider::OpenRouter => "OPENROUTER_API_KEY",
                AiProvider::Ollama => "", // Ollama doesn't need API key
            });

        let api_key = if !api_key_env.is_empty() {
            env::var(api_key_env)
                .map_err(|_| anyhow::anyhow!("Environment variable '{}' not set", api_key_env))?
        } else {
            String::new()
        };

        // Get Ollama base URL if using Ollama
        let ollama_base_url = config
            .get("api_base_url")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:11434");

        let mut df = data.as_dataframe()?;

        // Setup Handlebars for template rendering
        let handlebars = Handlebars::new();

        // Process each row
        let mut results = Vec::new();
        let height = df.height();

        for i in 0..height {
            // Build context from row
            let mut context = serde_json::Map::new();
            for col_name in df.get_column_names() {
                let column = df.column(col_name)?;
                let value = match column.dtype() {
                    DataType::String => {
                        let str_val = column.str()?.get(i).unwrap_or("");
                        serde_json::Value::String(str_val.to_string())
                    }
                    DataType::Int64 => {
                        let int_val = column.i64()?.get(i);
                        int_val
                            .map(serde_json::Value::from)
                            .unwrap_or(serde_json::Value::Null)
                    }
                    DataType::Float64 => {
                        let float_val = column.f64()?.get(i);
                        float_val
                            .map(serde_json::Value::from)
                            .unwrap_or(serde_json::Value::Null)
                    }
                    DataType::Boolean => {
                        let bool_val = column.bool()?.get(i);
                        bool_val
                            .map(serde_json::Value::from)
                            .unwrap_or(serde_json::Value::Null)
                    }
                    _ => serde_json::Value::Null,
                };
                context.insert(col_name.to_string(), value);
            }

            // Render prompt template
            let prompt = handlebars
                .render_template(prompt_template, &context)
                .map_err(|e| anyhow::anyhow!("Failed to render prompt template: {}", e))?;

            // Call appropriate provider
            let response = match provider {
                AiProvider::OpenAI => {
                    self.call_openai(&api_key, model, &prompt, max_tokens, temperature)
                        .await?
                }
                AiProvider::Anthropic => {
                    self.call_anthropic(&api_key, model, &prompt, max_tokens, temperature)
                        .await?
                }
                AiProvider::OpenRouter => {
                    self.call_openrouter(&api_key, model, &prompt, max_tokens, temperature)
                        .await?
                }
                AiProvider::Ollama => {
                    self.call_ollama(ollama_base_url, model, &prompt, max_tokens, temperature)
                        .await?
                }
            };

            results.push(response);
        }

        // Add results as new column
        let new_series = Series::new(output_column.into(), results);
        df.with_column(new_series)?;

        Ok(DataFormat::DataFrame(df))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("provider") {
            anyhow::bail!("AI requires 'provider' configuration");
        }

        if !config.contains_key("model") {
            anyhow::bail!("AI requires 'model' configuration");
        }

        if !config.contains_key("prompt") {
            anyhow::bail!("AI requires 'prompt' configuration");
        }

        if !config.contains_key("output_column") {
            anyhow::bail!("AI requires 'output_column' configuration");
        }

        Ok(())
    }
}
