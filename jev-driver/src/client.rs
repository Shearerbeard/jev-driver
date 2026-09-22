//! The Jev client: transport, retry (429/529), cancellation, and typed
//! evaluation. The only module that talks HTTP.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::answer::{Answers, FromAnswers};
use crate::error::{JevError, JevResult};
use crate::question::Decision;
use crate::wire::{WireRequestBody, WireResponse};

/// Default model pin. The moving `jev-latest` alias can silently change
/// thresholds; pin by default.
pub const DEFAULT_MODEL: &str = "jev-1.13.0";

const DEFAULT_BASE_URL: &str = "https://api.typesafe.ai";
const SYSTEMONE_PATH: &str = "v1/systemone";

/// Retry policy for 429/529 responses.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum retries before surfacing the error.
    pub max_retries: u32,
    /// First backoff delay.
    pub initial_interval: Duration,
    /// Backoff ceiling.
    pub max_interval: Duration,
    /// Backoff multiplier. Values below 1.0 or non-finite (NaN) are
    /// sanitized to 1.0 (constant interval) by [`RetryConfig::delay_for`].
    pub multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_interval: Duration::from_millis(500),
            max_interval: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Delay before retry `attempt` (0-based). Never panics: an
    /// undersized `max_interval` floors the delay, a `multiplier` below
    /// 1.0 or NaN is treated as 1.0, a non-finite product clamps to
    /// `max_interval`, and durations past `Duration::MAX` saturate at
    /// `max_interval` instead of tripping the float conversion.
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let factor = self.multiplier.max(1.0).powi(attempt.clamp(0, 16) as i32);
        let ceiling = self.max_interval.as_secs_f64();
        let floor = 0.001_f64.min(ceiling);
        let raw = self.initial_interval.as_secs_f64() * factor;
        // NaN arises only from 0 * inf; treat pathological config as the
        // ceiling. `try_from_secs_f64` rejects values `Duration` cannot
        // represent (e.g. extreme intervals rounding past `Duration::MAX`);
        // saturate at `max_interval` rather than panicking.
        let delay_secs = if raw.is_nan() { ceiling } else { raw };
        Duration::try_from_secs_f64(delay_secs.clamp(floor, ceiling)).unwrap_or(self.max_interval)
    }
}

/// Client configuration.
#[derive(Debug, Clone)]
pub struct JevConfig {
    /// API key (`TYPESAFE_API_KEY`).
    pub api_key: String,
    /// API base URL (defaults to `https://api.typesafe.ai`).
    pub base_url: Url,
    /// Pinned model id.
    pub model: String,
    /// Request timeout.
    pub timeout: Duration,
    /// Retry policy.
    pub retry: RetryConfig,
}

impl JevConfig {
    /// Builds a config from the environment: `TYPESAFE_API_KEY` (required),
    /// `TYPESAFE_BASE_URL` and `TYPESAFE_MODEL` (optional). Loads a
    /// workspace `.env` if present.
    pub fn from_env() -> JevResult<Self> {
        let _dotenv = dotenvy::dotenv();
        let api_key = std::env::var("TYPESAFE_API_KEY").map_err(|_| JevError::Unauthorized)?;
        Ok(Self {
            api_key,
            base_url: match std::env::var("TYPESAFE_BASE_URL") {
                Ok(raw) => raw.parse().map_err(|_| {
                    JevError::Transport(format!("invalid TYPESAFE_BASE_URL: {raw}"))
                })?,
                Err(_) => Url::parse(DEFAULT_BASE_URL)
                    .map_err(|e| JevError::Transport(format!("invalid default base URL: {e}")))?,
            },
            model: std::env::var("TYPESAFE_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_owned()),
            timeout: Duration::from_secs(30),
            retry: RetryConfig::default(),
        })
    }
}

/// Pluggable transport so tests never touch the network.
#[async_trait]
pub trait Transport: Send + Sync {
    /// POSTs the serialized request body to the systemone endpoint.
    async fn post_systemone(&self, body: String) -> JevResult<WireResponse>;
}

/// Production transport over reqwest.
pub struct ReqwestTransport {
    http: reqwest::Client,
    url: Url,
    api_key: String,
}

#[async_trait]
impl Transport for ReqwestTransport {
    async fn post_systemone(&self, body: String) -> JevResult<WireResponse> {
        let response = self
            .http
            .post(self.url.clone())
            .bearer_auth(&self.api_key)
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| JevError::Transport(format!("request failed: {e}")))?;
        let status = response.status();
        if status.is_success() {
            return response
                .json::<WireResponse>()
                .await
                .map_err(|e| JevError::Transport(format!("invalid response body: {e}")));
        }
        let text = response
            .text()
            .await
            .map_err(|e| JevError::Transport(format!("failed reading error body: {e}")))?;
        if status.as_u16() == 401 {
            return Err(JevError::Unauthorized);
        }
        if status.as_u16() == 422 {
            return Err(JevError::InvalidRequest(text));
        }
        if status.as_u16() == 429 {
            return Err(JevError::RateLimited { retries: 0 });
        }
        if status.as_u16() == 529 {
            return Err(JevError::Overloaded { retries: 0 });
        }
        Err(JevError::Transport(format!(
            "unexpected status {status}: {text}"
        )))
    }
}

/// The client. Cheap to clone (`Arc` internally); clones share transport
/// and configuration. Backoff honors neither `Retry-After` headers nor
/// jitter in 0.1.0.
#[derive(Clone)]
pub struct JevClient {
    transport: Arc<dyn Transport>,
    model: String,
    retry: RetryConfig,
}

impl JevClient {
    /// Builds a production client from an explicit config.
    pub fn new(config: JevConfig) -> JevResult<Self> {
        let http = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| JevError::Transport(format!("http client init failed: {e}")))?;
        let url = config
            .base_url
            .join(SYSTEMONE_PATH)
            .map_err(|e| JevError::Transport(format!("invalid base URL join: {e}")))?;
        Ok(Self {
            transport: Arc::new(ReqwestTransport {
                http,
                url,
                api_key: config.api_key,
            }),
            model: config.model,
            retry: config.retry,
        })
    }

    /// Builds a production client from the environment.
    pub fn from_env() -> JevResult<Self> {
        Self::new(JevConfig::from_env()?)
    }

    /// Test-only: runs against a scripted [`Transport`](crate::fake::FakeTransport).
    #[cfg(feature = "test-support")]
    pub fn with_transport(
        transport: Arc<dyn Transport>,
        model: impl Into<String>,
        retry: RetryConfig,
    ) -> Self {
        Self {
            transport,
            model: model.into(),
            retry,
        }
    }

    /// Evaluates a decision and parses the answers into a derived typed
    /// set (strict).
    pub async fn evaluate<T: FromAnswers>(&self, decision: &Decision) -> JevResult<T> {
        let answers = self.evaluate_raw(decision).await?;
        T::from_answers(&answers)
    }

    /// Evaluates a decision, returning validated dynamic answers.
    pub async fn evaluate_raw(&self, decision: &Decision) -> JevResult<Answers> {
        self.evaluate_raw_with(decision, None).await
    }

    /// Evaluates with a cancellation token. The token is checked before
    /// the first attempt and between retries; an already-in-flight HTTP
    /// request is not abortable in 0.1.0.
    pub async fn evaluate_raw_with_cancellation(
        &self,
        decision: &Decision,
        token: &CancellationToken,
    ) -> JevResult<Answers> {
        if token.is_cancelled() {
            return Err(JevError::Cancelled);
        }
        self.evaluate_raw_with(decision, Some(token)).await
    }

    #[allow(
        clippy::wildcard_enum_match_arm,
        reason = "only the two retryable variants can reach this match; the wildcard re-wraps the error verbatim and stays unreachable while the retryable guard above returns other variants early"
    )]
    async fn evaluate_raw_with(
        &self,
        decision: &Decision,
        token: Option<&CancellationToken>,
    ) -> JevResult<Answers> {
        let body = serde_json::to_string(&WireRequestBody {
            state: decision.state(),
            model: &self.model,
            questions: decision.questions(),
        })?;
        let mut attempt: u32 = 0;
        loop {
            match self.transport.post_systemone(body.clone()).await {
                Ok(response) => return Answers::from_wire(response, decision),
                Err(e) => {
                    if !matches!(
                        e,
                        JevError::RateLimited { .. } | JevError::Overloaded { .. }
                    ) {
                        return Err(e);
                    }
                    if attempt >= self.retry.max_retries {
                        return Err(match e {
                            JevError::RateLimited { .. } => {
                                JevError::RateLimited { retries: attempt }
                            }
                            JevError::Overloaded { .. } => {
                                JevError::Overloaded { retries: attempt }
                            }
                            other => other,
                        });
                    }
                    let delay = self.retry.delay_for(attempt);
                    tracing::warn!(
                        attempt,
                        delay_ms = delay.as_millis() as u64,
                        "retryable error, backing off"
                    );
                    attempt += 1;
                    match token {
                        Some(t) => {
                            tokio::select! {
                                biased;
                                () = t.cancelled() => return Err(JevError::Cancelled),
                                () = tokio::time::sleep(delay) => {}
                            }
                        }
                        None => tokio::time::sleep(delay).await,
                    }
                }
            }
        }
    }
}
