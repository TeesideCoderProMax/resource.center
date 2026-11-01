use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;

/// Versioned v1 notification request contract exchanged between the Loan outbox publisher
/// and the independent Notifications worker over Azure Service Bus.
///
/// Matches `docs/architecture/2026-10-25-resource-center-handoff.md`.
/// Represents a request to deliver a notification, not a fact that delivery occurred.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRequestV1 {
    pub event_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub version: u32,
    pub occurred_at: DateTime<Utc>,
    pub correlation_id: String,
    pub recipient: String,
    pub device_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthetic_probe: Option<bool>,
}

pub const EVENT_TYPE_AVAILABILITY_REQUESTED: &str = "DeviceAvailabilityNotificationRequested";
pub const EVENT_TYPE_COLLECTION_RECORDED: &str = "DeviceCollectionConfirmationRequested";
pub const EVENT_TYPE_RETURN_RECORDED: &str = "DeviceReturnConfirmationRequested";
pub const CONTRACT_VERSION_V1: u32 = 1;

#[derive(Debug, Error)]
pub enum ContractValidationError {
    #[error("unsupported contract version: {0} (expected 1)")]
    UnsupportedVersion(u32),
    #[error("unsupported event type: {0}")]
    UnsupportedEventType(String),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

impl NotificationRequestV1 {
    pub fn validate(&self) -> Result<(), ContractValidationError> {
        if self.version != CONTRACT_VERSION_V1 {
            return Err(ContractValidationError::UnsupportedVersion(self.version));
        }
        match self.event_type.as_str() {
            EVENT_TYPE_AVAILABILITY_REQUESTED
            | EVENT_TYPE_COLLECTION_RECORDED
            | EVENT_TYPE_RETURN_RECORDED => {}
            other => {
                return Err(ContractValidationError::UnsupportedEventType(
                    other.to_string(),
                ))
            }
        }
        if self.event_id.trim().is_empty() {
            return Err(ContractValidationError::MissingField("eventId"));
        }
        if self.correlation_id.trim().is_empty() {
            return Err(ContractValidationError::MissingField("correlationId"));
        }
        if self.recipient.trim().is_empty() {
            return Err(ContractValidationError::MissingField("recipient"));
        }
        if self.device_model.trim().is_empty() {
            return Err(ContractValidationError::MissingField("deviceModel"));
        }
        Ok(())
    }

    /// Redacts an email address for logs and non-delivery diagnostic telemetry so PII
    /// is never written in plain text to shared logs.
    pub fn redacted_recipient(&self) -> String {
        redact_email(&self.recipient)
    }
}

pub fn redact_email(email: &str) -> String {
    let trimmed = email.trim();
    if let Some((local, domain)) = trimmed.split_once('@') {
        let prefix = local.chars().next().unwrap_or('*');
        format!("{prefix}***@{domain}")
    } else {
        "redacted".to_string()
    }
}

pub fn init_json_tracing(service_name: &str) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .json()
        .with_env_filter(filter)
        .with_current_span(false)
        .with_target(true)
        .try_init();
    tracing::info!(
        service = service_name,
        "structured JSON logging initialized"
    );
}

#[derive(Debug, Error)]
pub enum ManagedIdentityError {
    #[error("managed identity HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("managed identity token endpoint rejected request ({status}): {body}")]
    Status { status: u16, body: String },
    #[error("managed identity environment not configured and no static token fallback set")]
    NotConfigured,
}

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    expires_at: Instant,
}

/// Acquires and caches Microsoft Entra access tokens for a specific User-Assigned Managed Identity
/// inside Azure Container Apps (`IDENTITY_ENDPOINT` + `IDENTITY_HEADER`) or via Azure IMDS.
#[derive(Clone)]
pub struct ManagedIdentityTokenProvider {
    client_id: String,
    http: reqwest::Client,
    cache: Arc<RwLock<std::collections::HashMap<String, CachedToken>>>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_on: Option<String>,
    #[serde(default)]
    expires_in: Option<String>,
}

impl ManagedIdentityTokenProvider {
    pub fn new(client_id: impl Into<String>) -> Result<Self, ManagedIdentityError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            client_id: client_id.into(),
            http,
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Returns a valid Bearer token for the requested resource URI (e.g.,
    /// `https://ossrdbms-aad.database.windows.net`, `https://servicebus.azure.net`,
    /// `https://cosmos.azure.com`, or `https://vault.azure.net`).
    pub async fn get_token(&self, resource: &str) -> Result<String, ManagedIdentityError> {
        let resource_trimmed = resource.trim_end_matches("/.default").trim_end_matches('/');
        {
            let guard = self.cache.read().await;
            if let Some(cached) = guard.get(resource_trimmed) {
                if Instant::now() + Duration::from_secs(120) < cached.expires_at {
                    return Ok(cached.token.clone());
                }
            }
        }

        let fetched = self.fetch_fresh_token(resource_trimmed).await?;
        let mut guard = self.cache.write().await;
        guard.insert(resource_trimmed.to_string(), fetched.clone());
        Ok(fetched.token)
    }

    async fn fetch_fresh_token(&self, resource: &str) -> Result<CachedToken, ManagedIdentityError> {
        if let Ok(static_override) = std::env::var("AZURE_STATIC_TOKEN_OVERRIDE") {
            if !static_override.trim().is_empty() {
                return Ok(CachedToken {
                    token: static_override,
                    expires_at: Instant::now() + Duration::from_secs(300),
                });
            }
        }

        if let (Ok(endpoint), Ok(header)) = (
            std::env::var("IDENTITY_ENDPOINT"),
            std::env::var("IDENTITY_HEADER"),
        ) {
            let resp = self
                .http
                .get(&endpoint)
                .header("X-IDENTITY-HEADER", header)
                .query(&[
                    ("api-version", "2019-08-01"),
                    ("resource", resource),
                    ("client_id", self.client_id.as_str()),
                ])
                .send()
                .await?;
            let status = resp.status().as_u16();
            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(ManagedIdentityError::Status { status, body });
            }
            let parsed: TokenResponse = resp.json().await?;
            let ttl = Self::compute_ttl(&parsed);
            return Ok(CachedToken {
                token: parsed.access_token,
                expires_at: Instant::now() + ttl,
            });
        }

        // Fallback to Azure IMDS endpoint if running in a VM/container without IDENTITY_ENDPOINT
        if std::env::var("USE_AZURE_IMDS").unwrap_or_default() == "true" {
            let imds_url = "http://169.254.169.254/metadata/identity/oauth2/token";
            let resp = self
                .http
                .get(imds_url)
                .header("Metadata", "true")
                .query(&[
                    ("api-version", "2018-02-01"),
                    ("resource", resource),
                    ("client_id", self.client_id.as_str()),
                ])
                .send()
                .await?;
            let status = resp.status().as_u16();
            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(ManagedIdentityError::Status { status, body });
            }
            let parsed: TokenResponse = resp.json().await?;
            let ttl = Self::compute_ttl(&parsed);
            return Ok(CachedToken {
                token: parsed.access_token,
                expires_at: Instant::now() + ttl,
            });
        }

        Err(ManagedIdentityError::NotConfigured)
    }

    fn compute_ttl(parsed: &TokenResponse) -> Duration {
        if let Some(ref exp_in) = parsed.expires_in {
            if let Ok(secs) = exp_in.parse::<u64>() {
                return Duration::from_secs(secs.saturating_sub(60).max(60));
            }
        }
        if let Some(ref exp_on) = parsed.expires_on {
            if let Ok(epoch) = exp_on.parse::<i64>() {
                let now = Utc::now().timestamp();
                if epoch > now {
                    return Duration::from_secs((epoch - now) as u64);
                }
            }
        }
        Duration::from_secs(900)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_v1_notification_contract() {
        let req = NotificationRequestV1 {
            event_id: "evt-123".to_string(),
            event_type: EVENT_TYPE_AVAILABILITY_REQUESTED.to_string(),
            version: 1,
            occurred_at: Utc::now(),
            correlation_id: "corr-456".to_string(),
            recipient: "student@example.edu".to_string(),
            device_model: "Dell Latitude 7440".to_string(),
            synthetic_probe: Some(true),
        };
        assert!(req.validate().is_ok());
        assert_eq!(req.redacted_recipient(), "s***@example.edu");
    }

    #[test]
    fn rejects_unsupported_contract_version() {
        let req = NotificationRequestV1 {
            event_id: "evt-123".to_string(),
            event_type: EVENT_TYPE_AVAILABILITY_REQUESTED.to_string(),
            version: 2,
            occurred_at: Utc::now(),
            correlation_id: "corr-456".to_string(),
            recipient: "student@example.edu".to_string(),
            device_model: "Dell Latitude 7440".to_string(),
            synthetic_probe: None,
        };
        assert!(matches!(
            req.validate(),
            Err(ContractValidationError::UnsupportedVersion(2))
        ));
    }
}
