use anyhow::Result;
use async_trait::async_trait;
use handlebars::Handlebars;
use reqwest::Client;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::core::traits::{DataFormat, Transform};

/// HTTP Fetch Transform
/// Fetches data from HTTP APIs using input data as context for templated requests
pub struct HttpFetchTransform {
    client: Client,
    handlebars: Handlebars<'static>,
}

impl HttpFetchTransform {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            handlebars: Handlebars::new(),
        }
    }
}

#[async_trait]
impl Transform for HttpFetchTransform {
    async fn name(&self) -> &str {
        "http_fetch"
    }

    async fn apply(
        &self,
        data: DataFormat,
        config: &Option<HashMap<String, toml::Value>>,
    ) -> Result<DataFormat> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("http_fetch requires configuration"))?;

        // Parse configuration
        let url_template = config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' in http_fetch config"))?;

        let method = config
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET");

        let mode = config
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("per_row");

        let result_field = config
            .get("result_field")
            .and_then(|v| v.as_str())
            .unwrap_or("http_result");

        let body_template = config.get("body").and_then(|v| v.as_str());

        // Get headers
        let mut headers = HashMap::new();
        if let Some(headers_config) = config.get("headers") {
            if let Some(headers_table) = headers_config.as_table() {
                for (key, value) in headers_table {
                    if let Some(val_str) = value.as_str() {
                        headers.insert(key.clone(), val_str.to_string());
                    }
                }
            }
        }

        // Convert input data to records
        let records = data.as_record_batch()?;

        match mode {
            "per_row" => {
                self.fetch_per_row(
                    records,
                    url_template,
                    method,
                    body_template,
                    &headers,
                    result_field,
                )
                .await
            }
            "batch" => {
                self.fetch_batch(
                    records,
                    url_template,
                    method,
                    body_template,
                    &headers,
                    result_field,
                )
                .await
            }
            _ => Err(anyhow::anyhow!(
                "Invalid mode '{}'. Must be 'per_row' or 'batch'",
                mode
            )),
        }
    }

    async fn validate_config(
        &self,
        config: &Option<HashMap<String, toml::Value>>,
    ) -> Result<()> {
        let config = config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("http_fetch requires configuration"))?;

        // Validate required fields
        if !config.contains_key("url") {
            anyhow::bail!("http_fetch requires 'url' field");
        }

        // Validate method if present
        if let Some(method) = config.get("method").and_then(|v| v.as_str()) {
            let valid_methods = ["GET", "POST", "PUT", "PATCH", "DELETE"];
            if !valid_methods.contains(&method) {
                anyhow::bail!(
                    "Invalid HTTP method '{}'. Must be one of: {:?}",
                    method,
                    valid_methods
                );
            }
        }

        // Validate mode if present
        if let Some(mode) = config.get("mode").and_then(|v| v.as_str()) {
            if mode != "per_row" && mode != "batch" {
                anyhow::bail!("Invalid mode '{}'. Must be 'per_row' or 'batch'", mode);
            }
        }

        Ok(())
    }
}

impl HttpFetchTransform {
    /// Fetch data for each row individually
    async fn fetch_per_row(
        &self,
        records: Vec<HashMap<String, JsonValue>>,
        url_template: &str,
        method: &str,
        body_template: Option<&str>,
        headers: &HashMap<String, String>,
        result_field: &str,
    ) -> Result<DataFormat> {
        let mut result_records = Vec::new();

        for (index, record) in records.iter().enumerate() {
            debug!("Processing row {}", index);

            // Render URL template
            let url = self.handlebars.render_template(url_template, record)?;
            debug!("Rendered URL: {}", url);

            // Render body template if present
            let body = if let Some(template) = body_template {
                Some(self.handlebars.render_template(template, record)?)
            } else {
                None
            };

            // Make HTTP request
            match self.make_request(&url, method, body.as_deref(), headers).await {
                Ok(response_data) => {
                    // Clone the original record and add the result
                    let mut new_record = record.clone();
                    new_record.insert(result_field.to_string(), response_data);
                    result_records.push(new_record);
                }
                Err(e) => {
                    warn!("HTTP request failed for row {}: {}", index, e);
                    // Add null result
                    let mut new_record = record.clone();
                    new_record.insert(result_field.to_string(), JsonValue::Null);
                    result_records.push(new_record);
                }
            }
        }

        info!(
            "Completed {} HTTP requests, {} successful",
            records.len(),
            result_records
                .iter()
                .filter(|r| !r.get(result_field).unwrap().is_null())
                .count()
        );

        Ok(DataFormat::RecordBatch(result_records))
    }

    /// Fetch data in batch mode (single request with all data)
    async fn fetch_batch(
        &self,
        records: Vec<HashMap<String, JsonValue>>,
        url_template: &str,
        method: &str,
        body_template: Option<&str>,
        headers: &HashMap<String, String>,
        result_field: &str,
    ) -> Result<DataFormat> {
        // Create context with all records
        let context = json!({ "records": records });

        // Render URL
        let url = self.handlebars.render_template(url_template, &context)?;

        // Render body
        let body = if let Some(template) = body_template {
            Some(self.handlebars.render_template(template, &context)?)
        } else {
            None
        };

        // Make single request
        let response_data = self.make_request(&url, method, body.as_deref(), headers).await?;

        // Add result to all records
        let mut result_records = records.clone();
        for record in &mut result_records {
            record.insert(result_field.to_string(), response_data.clone());
        }

        Ok(DataFormat::RecordBatch(result_records))
    }

    /// Make HTTP request
    async fn make_request(
        &self,
        url: &str,
        method: &str,
        body: Option<&str>,
        headers: &HashMap<String, String>,
    ) -> Result<JsonValue> {
        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "PATCH" => self.client.patch(url),
            "DELETE" => self.client.delete(url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
        };

        // Add headers
        for (key, value) in headers {
            request = request.header(key, value);
        }

        // Add body if present
        if let Some(body_str) = body {
            request = request
                .header("Content-Type", "application/json")
                .body(body_str.to_string());
        }

        // Execute request
        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            anyhow::bail!("HTTP request failed with status: {}", status);
        }

        // Parse response as JSON
        let text = response.text().await?;
        let json: JsonValue = serde_json::from_str(&text)
            .unwrap_or_else(|_| JsonValue::String(text));

        Ok(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_fetch_config_validation() {
        let transform = HttpFetchTransform::new();

        // Missing URL
        let config = Some(HashMap::new());
        assert!(transform.validate_config(&config).await.is_err());

        // Valid config
        let mut config = HashMap::new();
        config.insert("url".to_string(), toml::Value::String("http://example.com".to_string()));
        assert!(transform.validate_config(&Some(config)).await.is_ok());
    }

    #[tokio::test]
    async fn test_url_template_rendering() {
        let transform = HttpFetchTransform::new();
        let mut record = HashMap::new();
        record.insert("id".to_string(), json!(123));
        record.insert("name".to_string(), json!("Alice"));

        let url = transform
            .handlebars
            .render_template("https://api.example.com/users/{{ id }}/posts", &record)
            .unwrap();

        assert_eq!(url, "https://api.example.com/users/123/posts");
    }
}
