use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "strategy", rename_all = "lowercase")]
#[derive(Default)]
pub enum ErrorStrategy {
    #[default]
    Stop,
    Continue,
    Retry {
        #[serde(default = "default_max_retries")]
        max_retries: u32,
        #[serde(default = "default_retry_delay_seconds")]
        retry_delay_seconds: u64,
    },
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay_seconds() -> u64 {
    5
}

impl ErrorStrategy {
    /// Execute an operation with the configured error handling strategy
    pub async fn execute<F, Fut, T>(&self, operation_name: &str, operation: F) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        match self {
            ErrorStrategy::Stop => {
                // Execute once, fail immediately on error
                operation().await
            }
            ErrorStrategy::Continue => {
                // Execute once, log error but return Ok(None) equivalent
                // Note: This requires the caller to handle Option<T>
                // For now, we'll just execute and propagate the error
                // The caller will catch and decide whether to continue
                operation().await
            }
            ErrorStrategy::Retry {
                max_retries,
                retry_delay_seconds,
            } => {
                let mut attempts = 0;
                let max_attempts = *max_retries + 1; // Initial attempt + retries

                loop {
                    attempts += 1;

                    match operation().await {
                        Ok(result) => {
                            if attempts > 1 {
                                warn!(
                                    "Operation '{}' succeeded on attempt {}/{}",
                                    operation_name, attempts, max_attempts
                                );
                            }
                            return Ok(result);
                        }
                        Err(e) => {
                            if attempts >= max_attempts {
                                error!(
                                    "Operation '{}' failed after {} attempts: {}",
                                    operation_name, attempts, e
                                );
                                return Err(e);
                            }

                            warn!(
                                "Operation '{}' failed on attempt {}/{}: {}. Retrying in {} seconds...",
                                operation_name, attempts, max_attempts, e, retry_delay_seconds
                            );

                            sleep(Duration::from_secs(*retry_delay_seconds)).await;
                        }
                    }
                }
            }
        }
    }

    /// Check if this strategy should continue on error
    pub fn should_continue_on_error(&self) -> bool {
        matches!(self, ErrorStrategy::Continue)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::Mutex;

    #[tokio::test]
    async fn test_stop_strategy_fails_immediately() {
        let strategy = ErrorStrategy::Stop;
        let call_count = Arc::new(Mutex::new(0));

        let count_clone = call_count.clone();
        let result = strategy
            .execute("test", move || {
                let count_clone = count_clone.clone();
                async move {
                    *count_clone.lock().unwrap() += 1;
                    Err::<(), _>(anyhow::anyhow!("test error"))
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(*call_count.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn test_retry_strategy_retries() {
        let strategy = ErrorStrategy::Retry {
            max_retries: 2,
            retry_delay_seconds: 0, // No delay for testing
        };
        let call_count = Arc::new(Mutex::new(0));

        let count_clone = call_count.clone();
        let result = strategy
            .execute("test", move || {
                let count_clone = count_clone.clone();
                async move {
                    let mut count = count_clone.lock().unwrap();
                    *count += 1;
                    let current_count = *count;
                    drop(count);

                    if current_count < 2 {
                        Err(anyhow::anyhow!("test error"))
                    } else {
                        Ok(42)
                    }
                }
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*call_count.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_retry_strategy_fails_after_max_attempts() {
        let strategy = ErrorStrategy::Retry {
            max_retries: 2,
            retry_delay_seconds: 0,
        };
        let call_count = Arc::new(Mutex::new(0));

        let count_clone = call_count.clone();
        let result = strategy
            .execute("test", move || {
                let count_clone = count_clone.clone();
                async move {
                    *count_clone.lock().unwrap() += 1;
                    Err::<(), _>(anyhow::anyhow!("test error"))
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(*call_count.lock().unwrap(), 3); // Initial + 2 retries
    }

}
