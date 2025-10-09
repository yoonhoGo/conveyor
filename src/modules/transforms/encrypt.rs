use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes128Gcm, Aes256Gcm, Nonce as AesNonce,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaNonce};
use polars::prelude::*;
use rand::RngCore;
use std::collections::HashMap;

use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata,
};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct EncryptTransform;

#[derive(Debug, Clone, Copy)]
enum Algorithm {
    Aes128Gcm,
    Aes256Gcm,
    ChaCha20Poly1305,
}

impl Algorithm {
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "aes-128-gcm" | "aes128gcm" => Ok(Algorithm::Aes128Gcm),
            "aes-256-gcm" | "aes256gcm" => Ok(Algorithm::Aes256Gcm),
            "chacha20-poly1305" | "chacha20poly1305" => Ok(Algorithm::ChaCha20Poly1305),
            _ => anyhow::bail!(
                "Unknown encryption algorithm: '{}'. Supported: aes-128-gcm, aes-256-gcm, chacha20-poly1305",
                s
            ),
        }
    }

    fn key_size(&self) -> usize {
        match self {
            Algorithm::Aes128Gcm => 16,
            Algorithm::Aes256Gcm => 32,
            Algorithm::ChaCha20Poly1305 => 32,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Algorithm::Aes128Gcm => "AES-128-GCM",
            Algorithm::Aes256Gcm => "AES-256-GCM",
            Algorithm::ChaCha20Poly1305 => "ChaCha20-Poly1305",
        }
    }
}

#[async_trait]
impl Stage for EncryptTransform {
    fn name(&self) -> &str {
        "encrypt.apply"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert(
            "column".to_string(),
            toml::Value::String("email".to_string()),
        );
        example1.insert(
            "key".to_string(),
            toml::Value::String("my-secret-key-32-bytes-long!!".to_string()),
        );
        example1.insert(
            "output_column".to_string(),
            toml::Value::String("encrypted_email".to_string()),
        );

        let mut example2 = HashMap::new();
        example2.insert(
            "column".to_string(),
            toml::Value::String("credit_card".to_string()),
        );
        example2.insert(
            "key".to_string(),
            toml::Value::String("${ENCRYPTION_KEY}".to_string()),
        );

        let mut example3 = HashMap::new();
        example3.insert("column".to_string(), toml::Value::String("ssn".to_string()));
        example3.insert(
            "key".to_string(),
            toml::Value::String("1234567890123456".to_string()),
        );
        example3.insert(
            "algorithm".to_string(),
            toml::Value::String("aes-128-gcm".to_string()),
        );

        StageMetadata::builder("encrypt.apply", StageCategory::Transform)
            .description("Encrypt column values using various encryption algorithms")
            .long_description(
                "Encrypts column values using authenticated encryption algorithms. \
                Supports AES-128-GCM, AES-256-GCM (default), and ChaCha20-Poly1305. \
                Each value is encrypted with a unique nonce, and the result is base64-encoded \
                in the format: nonce + ciphertext. \
                Key sizes: AES-128-GCM (16 bytes), AES-256-GCM (32 bytes), ChaCha20-Poly1305 (32 bytes). \
                This is a secure encryption method suitable for sensitive data like PII, \
                passwords, or financial information.",
            )
            .parameter(ConfigParameter::required(
                "column",
                ParameterType::String,
                "Name of the column containing data to encrypt",
            ))
            .parameter(ConfigParameter::required(
                "key",
                ParameterType::String,
                "Encryption key (16 bytes for AES-128, 32 bytes for AES-256/ChaCha20)",
            ))
            .parameter(
                ConfigParameter::optional(
                    "algorithm",
                    ParameterType::String,
                    "aes-256-gcm",
                    "Encryption algorithm to use",
                )
                .with_validation(ParameterValidation::allowed_values([
                    "aes-128-gcm",
                    "aes-256-gcm",
                    "chacha20-poly1305",
                ])),
            )
            .parameter(ConfigParameter::optional(
                "output_column",
                ParameterType::String,
                "[original]_encrypted",
                "Name of the output column for encrypted data",
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Encrypt email addresses with AES-256-GCM (default)",
                example1,
                Some("Encrypt email column and store in encrypted_email column"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Encrypt credit card numbers with env variable",
                example2,
                Some("Encrypt credit_card column using key from environment variable"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Encrypt SSN with AES-128-GCM",
                example3,
                Some("Use AES-128-GCM algorithm with 16-byte key"),
            ))
            .tag("encrypt")
            .tag("security")
            .tag("transform")
            .tag("aes")
            .tag("gcm")
            .tag("chacha20")
            .build()
    }

    async fn execute(
        &self,
        inputs: HashMap<String, DataFormat>,
        config: &HashMap<String, toml::Value>,
    ) -> Result<DataFormat> {
        let data = inputs
            .into_values()
            .next()
            .ok_or_else(|| anyhow::anyhow!("Encrypt transform requires input data"))?;

        let column = config
            .get("column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Encrypt requires 'column' configuration"))?;

        let key = config
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Encrypt requires 'key' configuration"))?;

        let algorithm_str = config
            .get("algorithm")
            .and_then(|v| v.as_str())
            .unwrap_or("aes-256-gcm");

        let algorithm = Algorithm::from_str(algorithm_str)?;

        let default_output = format!("{}_encrypted", column);
        let output_column = config
            .get("output_column")
            .and_then(|v| v.as_str())
            .unwrap_or(&default_output);

        let df = data.as_dataframe()?;

        // Validate key length
        let expected_key_size = algorithm.key_size();
        if key.len() != expected_key_size {
            anyhow::bail!(
                "{} requires a key of exactly {} bytes, got {} bytes",
                algorithm.name(),
                expected_key_size,
                key.len()
            );
        }

        // Get column data as string
        let column_data = df
            .column(column)
            .with_context(|| format!("Column '{}' not found", column))?
            .str()
            .with_context(|| {
                format!("Column '{}' must be of string type for encryption", column)
            })?;

        // Encrypt each value based on algorithm
        let encrypted_values: Vec<Option<String>> = match algorithm {
            Algorithm::Aes128Gcm => {
                let key_bytes: [u8; 16] = key
                    .as_bytes()
                    .try_into()
                    .context("Failed to convert key to 16-byte array")?;
                let cipher = Aes128Gcm::new(&key_bytes.into());
                encrypt_values_aes(&cipher, column_data)?
            }
            Algorithm::Aes256Gcm => {
                let key_bytes: [u8; 32] = key
                    .as_bytes()
                    .try_into()
                    .context("Failed to convert key to 32-byte array")?;
                let cipher = Aes256Gcm::new(&key_bytes.into());
                encrypt_values_aes(&cipher, column_data)?
            }
            Algorithm::ChaCha20Poly1305 => {
                let key_bytes: [u8; 32] = key
                    .as_bytes()
                    .try_into()
                    .context("Failed to convert key to 32-byte array")?;
                let cipher = ChaCha20Poly1305::new(&key_bytes.into());
                encrypt_values_chacha(&cipher, column_data)?
            }
        };

        // Create new series with encrypted values
        let encrypted_series = Series::new(output_column.into(), encrypted_values);

        // Add encrypted column to DataFrame
        let mut result_df = df.clone();
        result_df
            .with_column(encrypted_series)
            .context("Failed to add encrypted column to DataFrame")?;

        Ok(DataFormat::DataFrame(result_df))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("column") {
            anyhow::bail!("Encrypt requires 'column' configuration");
        }

        if !config.contains_key("key") {
            anyhow::bail!("Encrypt requires 'key' configuration");
        }

        // Validate algorithm if provided
        if let Some(algorithm) = config.get("algorithm").and_then(|v| v.as_str()) {
            Algorithm::from_str(algorithm)?;
        }

        // Validate key length if provided (unless it's an env variable)
        if let Some(key) = config.get("key").and_then(|v| v.as_str()) {
            if !key.starts_with("${") {
                let algorithm_str = config
                    .get("algorithm")
                    .and_then(|v| v.as_str())
                    .unwrap_or("aes-256-gcm");
                let algorithm = Algorithm::from_str(algorithm_str)?;
                let expected_size = algorithm.key_size();

                if key.len() != expected_size {
                    anyhow::bail!(
                        "{} requires a key of exactly {} bytes, got {} bytes. \
                        Note: Use environment variables like ${{ENCRYPTION_KEY}} if the key is stored externally.",
                        algorithm.name(),
                        expected_size,
                        key.len()
                    );
                }
            }
        }

        Ok(())
    }
}

fn encrypt_values_aes<C>(cipher: &C, column_data: &StringChunked) -> Result<Vec<Option<String>>>
where
    C: Aead,
{
    column_data
        .into_iter()
        .map(|opt_str| {
            // Handle null values
            let value_str = match opt_str {
                Some(s) => s,
                None => return Ok(None),
            };

            // Generate random nonce (12 bytes for GCM)
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = AesNonce::from_slice(&nonce_bytes);

            // Encrypt
            let ciphertext = cipher
                .encrypt(nonce, value_str.as_bytes())
                .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

            // Combine nonce + ciphertext and encode as base64
            let mut combined = nonce_bytes.to_vec();
            combined.extend_from_slice(&ciphertext);
            let encoded =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &combined);

            Ok(Some(encoded))
        })
        .collect::<Result<Vec<_>>>()
}

fn encrypt_values_chacha(
    cipher: &ChaCha20Poly1305,
    column_data: &StringChunked,
) -> Result<Vec<Option<String>>> {
    column_data
        .into_iter()
        .map(|opt_str| {
            // Handle null values
            let value_str = match opt_str {
                Some(s) => s,
                None => return Ok(None),
            };

            // Generate random nonce (12 bytes for ChaCha20)
            let mut nonce_bytes = [0u8; 12];
            OsRng.fill_bytes(&mut nonce_bytes);
            let nonce = ChaNonce::from_slice(&nonce_bytes);

            // Encrypt
            let ciphertext = cipher
                .encrypt(nonce, value_str.as_bytes())
                .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

            // Combine nonce + ciphertext and encode as base64
            let mut combined = nonce_bytes.to_vec();
            combined.extend_from_slice(&ciphertext);
            let encoded =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &combined);

            Ok(Some(encoded))
        })
        .collect::<Result<Vec<_>>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use polars::df;

    #[tokio::test]
    async fn test_encrypt_aes256() {
        let transform = EncryptTransform;

        let df = df! {
            "id" => &[1, 2, 3],
            "email" => &["alice@example.com", "bob@example.com", "charlie@example.com"],
        }
        .unwrap();

        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("email".to_string()),
        );
        config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let result = transform.execute(inputs, &config).await.unwrap();
        let result_df = result.as_dataframe().unwrap();

        assert!(result_df.column("email_encrypted").is_ok());

        let encrypted = result_df.column("email_encrypted").unwrap();
        let encrypted_str = encrypted.str().unwrap();
        let first_encrypted = encrypted_str.get(0).unwrap();

        assert_ne!(first_encrypted, "alice@example.com");
        assert!(first_encrypted.len() > 20);
    }

    #[tokio::test]
    async fn test_encrypt_aes128() {
        let transform = EncryptTransform;

        let df = df! {
            "data" => &["secret1", "secret2"],
        }
        .unwrap();

        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("data".to_string()),
        );
        config.insert(
            "key".to_string(),
            toml::Value::String("1234567890123456".to_string()), // 16 bytes
        );
        config.insert(
            "algorithm".to_string(),
            toml::Value::String("aes-128-gcm".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let result = transform.execute(inputs, &config).await.unwrap();
        let result_df = result.as_dataframe().unwrap();

        assert!(result_df.column("data_encrypted").is_ok());
    }

    #[tokio::test]
    async fn test_encrypt_chacha20() {
        let transform = EncryptTransform;

        let df = df! {
            "data" => &["secret"],
        }
        .unwrap();

        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("data".to_string()),
        );
        config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()), // 32 bytes
        );
        config.insert(
            "algorithm".to_string(),
            toml::Value::String("chacha20-poly1305".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let result = transform.execute(inputs, &config).await.unwrap();
        let result_df = result.as_dataframe().unwrap();

        assert!(result_df.column("data_encrypted").is_ok());
    }

    #[tokio::test]
    async fn test_encrypt_custom_output_column() {
        let transform = EncryptTransform;

        let df = df! {
            "password" => &["secret123", "pass456"],
        }
        .unwrap();

        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("password".to_string()),
        );
        config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );
        config.insert(
            "output_column".to_string(),
            toml::Value::String("encrypted_pass".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let result = transform.execute(inputs, &config).await.unwrap();
        let result_df = result.as_dataframe().unwrap();

        assert!(result_df.column("encrypted_pass").is_ok());
    }

    #[tokio::test]
    async fn test_encrypt_invalid_key_length() {
        let transform = EncryptTransform;

        let df = df! {
            "data" => &["test"],
        }
        .unwrap();

        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("data".to_string()),
        );
        config.insert(
            "key".to_string(),
            toml::Value::String("short-key".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let result = transform.execute(inputs, &config).await;
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("32 bytes") || err_msg.contains("key"));
    }

    #[tokio::test]
    async fn test_encrypt_invalid_algorithm() {
        let transform = EncryptTransform;

        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("data".to_string()),
        );
        config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );
        config.insert(
            "algorithm".to_string(),
            toml::Value::String("invalid-algo".to_string()),
        );

        let result = transform.validate_config(&config).await;
        assert!(result.is_err());
    }
}
