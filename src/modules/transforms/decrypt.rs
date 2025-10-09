use aes_gcm::{aead::Aead, Aes128Gcm, Aes256Gcm, KeyInit, Nonce as AesNonce};
use anyhow::{Context, Result};
use async_trait::async_trait;
use chacha20poly1305::{ChaCha20Poly1305, Nonce as ChaNonce};
use polars::prelude::*;
use std::collections::HashMap;

use crate::core::metadata::{
    ConfigParameter, ParameterType, ParameterValidation, StageCategory, StageMetadata,
};
use crate::core::stage::Stage;
use crate::core::traits::DataFormat;

pub struct DecryptTransform;

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
                "Unknown decryption algorithm: '{}'. Supported: aes-128-gcm, aes-256-gcm, chacha20-poly1305",
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
impl Stage for DecryptTransform {
    fn name(&self) -> &str {
        "decrypt.apply"
    }

    fn metadata(&self) -> StageMetadata {
        let mut example1 = HashMap::new();
        example1.insert(
            "column".to_string(),
            toml::Value::String("encrypted_email".to_string()),
        );
        example1.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );
        example1.insert(
            "output_column".to_string(),
            toml::Value::String("email".to_string()),
        );

        let mut example2 = HashMap::new();
        example2.insert(
            "column".to_string(),
            toml::Value::String("encrypted_data".to_string()),
        );
        example2.insert(
            "key".to_string(),
            toml::Value::String("${ENCRYPTION_KEY}".to_string()),
        );

        let mut example3 = HashMap::new();
        example3.insert(
            "column".to_string(),
            toml::Value::String("encrypted_ssn".to_string()),
        );
        example3.insert(
            "key".to_string(),
            toml::Value::String("1234567890123456".to_string()),
        );
        example3.insert(
            "algorithm".to_string(),
            toml::Value::String("aes-128-gcm".to_string()),
        );

        StageMetadata::builder("decrypt.apply", StageCategory::Transform)
            .description("Decrypt column values encrypted with various algorithms")
            .long_description(
                "Decrypts column values that were encrypted using authenticated encryption algorithms. \
                Supports AES-128-GCM, AES-256-GCM (default), and ChaCha20-Poly1305. \
                The encryption key must match the algorithm and key used for encryption. \
                Expects base64-encoded input in the format: nonce + ciphertext. \
                Failed decryption attempts (invalid key, corrupted data) will result in null values by default. \
                Key sizes: AES-128-GCM (16 bytes), AES-256-GCM (32 bytes), ChaCha20-Poly1305 (32 bytes). \
                This transform is designed to work with data encrypted by the encrypt.apply transform.",
            )
            .parameter(ConfigParameter::required(
                "column",
                ParameterType::String,
                "Name of the column containing encrypted data to decrypt",
            ))
            .parameter(ConfigParameter::required(
                "key",
                ParameterType::String,
                "Decryption key (16 bytes for AES-128, 32 bytes for AES-256/ChaCha20, must match encryption key)",
            ))
            .parameter(
                ConfigParameter::optional(
                    "algorithm",
                    ParameterType::String,
                    "aes-256-gcm",
                    "Decryption algorithm to use (must match encryption algorithm)",
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
                "[original]_decrypted",
                "Name of the output column for decrypted data",
            ))
            .parameter(ConfigParameter::optional(
                "fail_on_error",
                ParameterType::Boolean,
                "false",
                "If true, fail the stage on decryption errors. If false, return null for failed decryptions.",
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Decrypt email addresses with AES-256-GCM (default)",
                example1,
                Some("Decrypt encrypted_email column and store in email column"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Decrypt with environment variable key",
                example2,
                Some("Decrypt data using key from environment variable"),
            ))
            .example(crate::core::metadata::ConfigExample::new(
                "Decrypt SSN with AES-128-GCM",
                example3,
                Some("Use AES-128-GCM algorithm with 16-byte key"),
            ))
            .tag("decrypt")
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
            .ok_or_else(|| anyhow::anyhow!("Decrypt transform requires input data"))?;

        let column = config
            .get("column")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Decrypt requires 'column' configuration"))?;

        let key = config
            .get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Decrypt requires 'key' configuration"))?;

        let algorithm_str = config
            .get("algorithm")
            .and_then(|v| v.as_str())
            .unwrap_or("aes-256-gcm");

        let algorithm = Algorithm::from_str(algorithm_str)?;

        let default_output = format!("{}_decrypted", column);
        let output_column = config
            .get("output_column")
            .and_then(|v| v.as_str())
            .unwrap_or(&default_output);

        let fail_on_error = config
            .get("fail_on_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

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
                format!("Column '{}' must be of string type for decryption", column)
            })?;

        // Decrypt each value based on algorithm
        let decrypted_values: Vec<Option<String>> = match algorithm {
            Algorithm::Aes128Gcm => {
                let key_bytes: [u8; 16] = key
                    .as_bytes()
                    .try_into()
                    .context("Failed to convert key to 16-byte array")?;
                let cipher = Aes128Gcm::new(&key_bytes.into());
                decrypt_values_aes(&cipher, column_data, fail_on_error)?
            }
            Algorithm::Aes256Gcm => {
                let key_bytes: [u8; 32] = key
                    .as_bytes()
                    .try_into()
                    .context("Failed to convert key to 32-byte array")?;
                let cipher = Aes256Gcm::new(&key_bytes.into());
                decrypt_values_aes(&cipher, column_data, fail_on_error)?
            }
            Algorithm::ChaCha20Poly1305 => {
                let key_bytes: [u8; 32] = key
                    .as_bytes()
                    .try_into()
                    .context("Failed to convert key to 32-byte array")?;
                let cipher = ChaCha20Poly1305::new(&key_bytes.into());
                decrypt_values_chacha(&cipher, column_data, fail_on_error)?
            }
        };

        // Create new series with decrypted values
        let decrypted_series = Series::new(output_column.into(), decrypted_values);

        // Add decrypted column to DataFrame
        let mut result_df = df.clone();
        result_df
            .with_column(decrypted_series)
            .context("Failed to add decrypted column to DataFrame")?;

        Ok(DataFormat::DataFrame(result_df))
    }

    async fn validate_config(&self, config: &HashMap<String, toml::Value>) -> Result<()> {
        if !config.contains_key("column") {
            anyhow::bail!("Decrypt requires 'column' configuration");
        }

        if !config.contains_key("key") {
            anyhow::bail!("Decrypt requires 'key' configuration");
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

fn decrypt_values_aes<C>(
    cipher: &C,
    column_data: &StringChunked,
    fail_on_error: bool,
) -> Result<Vec<Option<String>>>
where
    C: Aead,
{
    column_data
        .into_iter()
        .enumerate()
        .map(|(idx, opt_str)| {
            // Handle null values
            let encrypted_str = match opt_str {
                Some(s) => s,
                None => return Ok(None),
            };

            // Decode base64
            let combined = match base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                encrypted_str,
            ) {
                Ok(data) => data,
                Err(e) => {
                    if fail_on_error {
                        anyhow::bail!("Row {}: Failed to decode base64: {}", idx, e);
                    }
                    return Ok(None);
                }
            };

            // Extract nonce (first 12 bytes) and ciphertext (rest)
            if combined.len() < 12 {
                if fail_on_error {
                    anyhow::bail!("Row {}: Encrypted data too short", idx);
                }
                return Ok(None);
            }

            let (nonce_bytes, ciphertext) = combined.split_at(12);
            let nonce = AesNonce::from_slice(nonce_bytes);

            // Decrypt
            let plaintext = match cipher.decrypt(nonce, ciphertext) {
                Ok(data) => data,
                Err(e) => {
                    if fail_on_error {
                        anyhow::bail!("Row {}: Decryption failed: {}", idx, e);
                    }
                    return Ok(None);
                }
            };

            // Convert to string
            let decrypted_str = match String::from_utf8(plaintext) {
                Ok(s) => s,
                Err(e) => {
                    if fail_on_error {
                        anyhow::bail!("Row {}: Invalid UTF-8 in decrypted data: {}", idx, e);
                    }
                    return Ok(None);
                }
            };

            Ok(Some(decrypted_str))
        })
        .collect::<Result<Vec<_>>>()
}

fn decrypt_values_chacha(
    cipher: &ChaCha20Poly1305,
    column_data: &StringChunked,
    fail_on_error: bool,
) -> Result<Vec<Option<String>>> {
    column_data
        .into_iter()
        .enumerate()
        .map(|(idx, opt_str)| {
            // Handle null values
            let encrypted_str = match opt_str {
                Some(s) => s,
                None => return Ok(None),
            };

            // Decode base64
            let combined = match base64::Engine::decode(
                &base64::engine::general_purpose::STANDARD,
                encrypted_str,
            ) {
                Ok(data) => data,
                Err(e) => {
                    if fail_on_error {
                        anyhow::bail!("Row {}: Failed to decode base64: {}", idx, e);
                    }
                    return Ok(None);
                }
            };

            // Extract nonce (first 12 bytes) and ciphertext (rest)
            if combined.len() < 12 {
                if fail_on_error {
                    anyhow::bail!("Row {}: Encrypted data too short", idx);
                }
                return Ok(None);
            }

            let (nonce_bytes, ciphertext) = combined.split_at(12);
            let nonce = ChaNonce::from_slice(nonce_bytes);

            // Decrypt
            let plaintext = match cipher.decrypt(nonce, ciphertext) {
                Ok(data) => data,
                Err(e) => {
                    if fail_on_error {
                        anyhow::bail!("Row {}: Decryption failed: {}", idx, e);
                    }
                    return Ok(None);
                }
            };

            // Convert to string
            let decrypted_str = match String::from_utf8(plaintext) {
                Ok(s) => s,
                Err(e) => {
                    if fail_on_error {
                        anyhow::bail!("Row {}: Invalid UTF-8 in decrypted data: {}", idx, e);
                    }
                    return Ok(None);
                }
            };

            Ok(Some(decrypted_str))
        })
        .collect::<Result<Vec<_>>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::transforms::encrypt::EncryptTransform;
    use polars::df;

    #[tokio::test]
    async fn test_decrypt_aes256() {
        // First encrypt some data
        let encrypt_transform = EncryptTransform;
        let df = df! {
            "id" => &[1, 2, 3],
            "email" => &["alice@example.com", "bob@example.com", "charlie@example.com"],
        }
        .unwrap();

        let mut encrypt_config = HashMap::new();
        encrypt_config.insert(
            "column".to_string(),
            toml::Value::String("email".to_string()),
        );
        encrypt_config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let encrypted_result = encrypt_transform
            .execute(inputs, &encrypt_config)
            .await
            .unwrap();
        let encrypted_df = encrypted_result.as_dataframe().unwrap();

        // Now decrypt
        let decrypt_transform = DecryptTransform;
        let mut decrypt_config = HashMap::new();
        decrypt_config.insert(
            "column".to_string(),
            toml::Value::String("email_encrypted".to_string()),
        );
        decrypt_config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );
        decrypt_config.insert(
            "output_column".to_string(),
            toml::Value::String("email_decrypted".to_string()),
        );

        let mut decrypt_inputs = HashMap::new();
        decrypt_inputs.insert(
            "input".to_string(),
            DataFormat::DataFrame(encrypted_df.clone()),
        );

        let decrypted_result = decrypt_transform
            .execute(decrypt_inputs, &decrypt_config)
            .await
            .unwrap();
        let decrypted_df = decrypted_result.as_dataframe().unwrap();

        // Verify decrypted values match originals
        let original_emails = encrypted_df.column("email").unwrap();
        let decrypted_emails = decrypted_df.column("email_decrypted").unwrap();

        for i in 0..3 {
            let original = original_emails.str().unwrap().get(i).unwrap();
            let decrypted = decrypted_emails.str().unwrap().get(i).unwrap();
            assert_eq!(original, decrypted);
        }
    }

    #[tokio::test]
    async fn test_decrypt_aes128() {
        let encrypt_transform = EncryptTransform;
        let df = df! {
            "data" => &["secret1", "secret2"],
        }
        .unwrap();

        let mut encrypt_config = HashMap::new();
        encrypt_config.insert(
            "column".to_string(),
            toml::Value::String("data".to_string()),
        );
        encrypt_config.insert(
            "key".to_string(),
            toml::Value::String("1234567890123456".to_string()), // 16 bytes
        );
        encrypt_config.insert(
            "algorithm".to_string(),
            toml::Value::String("aes-128-gcm".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let encrypted_result = encrypt_transform
            .execute(inputs, &encrypt_config)
            .await
            .unwrap();
        let encrypted_df = encrypted_result.as_dataframe().unwrap();

        // Decrypt with same algorithm and key
        let decrypt_transform = DecryptTransform;
        let mut decrypt_config = HashMap::new();
        decrypt_config.insert(
            "column".to_string(),
            toml::Value::String("data_encrypted".to_string()),
        );
        decrypt_config.insert(
            "key".to_string(),
            toml::Value::String("1234567890123456".to_string()),
        );
        decrypt_config.insert(
            "algorithm".to_string(),
            toml::Value::String("aes-128-gcm".to_string()),
        );

        let mut decrypt_inputs = HashMap::new();
        decrypt_inputs.insert(
            "input".to_string(),
            DataFormat::DataFrame(encrypted_df.clone()),
        );

        let decrypted_result = decrypt_transform
            .execute(decrypt_inputs, &decrypt_config)
            .await
            .unwrap();
        let decrypted_df = decrypted_result.as_dataframe().unwrap();

        // Verify decrypted values match originals
        let original_data = encrypted_df.column("data").unwrap();
        let decrypted_data = decrypted_df.column("data_encrypted_decrypted").unwrap();

        for i in 0..2 {
            let original = original_data.str().unwrap().get(i).unwrap();
            let decrypted = decrypted_data.str().unwrap().get(i).unwrap();
            assert_eq!(original, decrypted);
        }
    }

    #[tokio::test]
    async fn test_decrypt_chacha20() {
        let encrypt_transform = EncryptTransform;
        let df = df! {
            "data" => &["secret"],
        }
        .unwrap();

        let mut encrypt_config = HashMap::new();
        encrypt_config.insert(
            "column".to_string(),
            toml::Value::String("data".to_string()),
        );
        encrypt_config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()), // 32 bytes
        );
        encrypt_config.insert(
            "algorithm".to_string(),
            toml::Value::String("chacha20-poly1305".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let encrypted_result = encrypt_transform
            .execute(inputs, &encrypt_config)
            .await
            .unwrap();
        let encrypted_df = encrypted_result.as_dataframe().unwrap();

        // Decrypt with same algorithm and key
        let decrypt_transform = DecryptTransform;
        let mut decrypt_config = HashMap::new();
        decrypt_config.insert(
            "column".to_string(),
            toml::Value::String("data_encrypted".to_string()),
        );
        decrypt_config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );
        decrypt_config.insert(
            "algorithm".to_string(),
            toml::Value::String("chacha20-poly1305".to_string()),
        );

        let mut decrypt_inputs = HashMap::new();
        decrypt_inputs.insert(
            "input".to_string(),
            DataFormat::DataFrame(encrypted_df.clone()),
        );

        let decrypted_result = decrypt_transform
            .execute(decrypt_inputs, &decrypt_config)
            .await
            .unwrap();
        let decrypted_df = decrypted_result.as_dataframe().unwrap();

        // Verify decrypted values match originals
        let original_data = encrypted_df.column("data").unwrap();
        let decrypted_data = decrypted_df.column("data_encrypted_decrypted").unwrap();

        let original = original_data.str().unwrap().get(0).unwrap();
        let decrypted = decrypted_data.str().unwrap().get(0).unwrap();
        assert_eq!(original, decrypted);
    }

    #[tokio::test]
    async fn test_decrypt_wrong_key() {
        // First encrypt
        let encrypt_transform = EncryptTransform;
        let df = df! {
            "data" => &["secret"],
        }
        .unwrap();

        let mut encrypt_config = HashMap::new();
        encrypt_config.insert(
            "column".to_string(),
            toml::Value::String("data".to_string()),
        );
        encrypt_config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let encrypted_result = encrypt_transform
            .execute(inputs, &encrypt_config)
            .await
            .unwrap();
        let encrypted_df = encrypted_result.as_dataframe().unwrap();

        // Try to decrypt with wrong key (should return null, not error)
        let decrypt_transform = DecryptTransform;
        let mut decrypt_config = HashMap::new();
        decrypt_config.insert(
            "column".to_string(),
            toml::Value::String("data_encrypted".to_string()),
        );
        decrypt_config.insert(
            "key".to_string(),
            toml::Value::String("XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".to_string()),
        );

        let mut decrypt_inputs = HashMap::new();
        decrypt_inputs.insert("input".to_string(), DataFormat::DataFrame(encrypted_df));

        let result = decrypt_transform
            .execute(decrypt_inputs, &decrypt_config)
            .await;
        assert!(result.is_ok());

        let result_df = result.unwrap().as_dataframe().unwrap();
        let decrypted = result_df.column("data_encrypted_decrypted").unwrap();

        // Should be null due to wrong key
        assert!(decrypted.is_null().all());
    }

    #[tokio::test]
    async fn test_decrypt_fail_on_error() {
        let df = df! {
            "data" => &["invalid-encrypted-data"],
        }
        .unwrap();

        let decrypt_transform = DecryptTransform;
        let mut config = HashMap::new();
        config.insert(
            "column".to_string(),
            toml::Value::String("data".to_string()),
        );
        config.insert(
            "key".to_string(),
            toml::Value::String("12345678901234567890123456789012".to_string()),
        );
        config.insert("fail_on_error".to_string(), toml::Value::Boolean(true));

        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), DataFormat::DataFrame(df));

        let result = decrypt_transform.execute(inputs, &config).await;
        assert!(result.is_err());
    }
}
