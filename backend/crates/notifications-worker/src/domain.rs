use chrono::{DateTime, Utc};
use rc_shared::{redact_email, NotificationRequestV1};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const LEASE_SECONDS: i64 = 60;
pub const EMAIL_TIMEOUT_SECONDS: u64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Claimed,
    Delivered,
    RetryableFailure,
    TerminalFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryAttemptRecord {
    pub attempt_number: u32,
    pub attempted_at: DateTime<Utc>,
    pub outcome: String,
    pub provider_message_id: Option<String>,
    pub error_detail: Option<String>,
}

/// Authoritative Cosmos DB document in `notifications` database, `jobs` container.
/// Partition key is `/eventId`, and `id` equals `eventId` so point reads/replaces
/// require only `(eventId, eventId)` and zero cross-partition queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationJobDocument {
    pub id: String,
    pub event_id: String,
    pub event_type: String,
    pub contract_version: u32,
    pub correlation_id: String,
    pub device_model: String,
    pub recipient_redacted: String,
    /// Temporary delivery address retained only while the job is in progress (`Claimed` or `RetryableFailure`).
    /// Erased (`None`) as soon as the job reaches `Delivered` or `TerminalFailure` for PII minimization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient_delivery: Option<String>,
    pub status: JobStatus,
    #[serde(default)]
    pub claim_token: Option<String>,
    #[serde(default)]
    pub lease_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub next_retry_at: Option<DateTime<Utc>>,
    pub attempt_count: u32,
    pub delivery_mode: String,
    pub synthetic_probe: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub attempts: Vec<DeliveryAttemptRecord>,
}

impl NotificationJobDocument {
    pub fn new_claimed(
        req: &NotificationRequestV1,
        delivery_mode: &str,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id: req.event_id.clone(),
            event_id: req.event_id.clone(),
            event_type: req.event_type.clone(),
            contract_version: req.version,
            correlation_id: req.correlation_id.clone(),
            device_model: req.device_model.clone(),
            recipient_redacted: redact_email(&req.recipient),
            recipient_delivery: Some(req.recipient.clone()),
            status: JobStatus::Claimed,
            claim_token: Some(Uuid::new_v4().to_string()),
            lease_until: Some(now + chrono::Duration::seconds(LEASE_SECONDS)),
            next_retry_at: None,
            attempt_count: 1,
            delivery_mode: delivery_mode.to_string(),
            synthetic_probe: req.synthetic_probe.unwrap_or(false),
            created_at: now,
            updated_at: now,
            delivered_at: None,
            last_error: None,
            attempts: Vec::new(),
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Delivered | JobStatus::TerminalFailure
        )
    }

    pub fn available_at(&self) -> DateTime<Utc> {
        match self.status {
            JobStatus::Claimed => self
                .lease_until
                .unwrap_or(self.updated_at + chrono::Duration::seconds(LEASE_SECONDS)),
            _ => self.next_retry_at.unwrap_or(self.updated_at),
        }
    }

    pub fn owns(&self, token: &str, now: DateTime<Utc>) -> bool {
        self.status == JobStatus::Claimed
            && self.claim_token.as_deref() == Some(token)
            && self.lease_until.is_some_and(|until| until > now)
    }

    pub fn mark_reclaimed(&mut self, now: DateTime<Utc>) {
        self.status = JobStatus::Claimed;
        self.attempt_count += 1;
        self.claim_token = Some(Uuid::new_v4().to_string());
        self.lease_until = Some(now + chrono::Duration::seconds(LEASE_SECONDS));
        self.next_retry_at = None;
        self.updated_at = now;
    }

    pub fn mark_delivered(
        &mut self,
        provider_message_id: String,
        delivery_mode: &str,
        now: DateTime<Utc>,
    ) {
        self.status = JobStatus::Delivered;
        self.claim_token = None;
        self.lease_until = None;
        self.next_retry_at = None;
        self.delivery_mode = delivery_mode.to_string();
        self.recipient_delivery = None;
        self.updated_at = now;
        self.delivered_at = Some(now);
        self.last_error = None;
        self.attempts.push(DeliveryAttemptRecord {
            attempt_number: self.attempt_count,
            attempted_at: now,
            outcome: "Delivered".to_string(),
            provider_message_id: Some(provider_message_id),
            error_detail: None,
        });
    }

    pub fn mark_failure(
        &mut self,
        error_detail: String,
        max_attempts: u32,
        now: DateTime<Utc>,
    ) -> JobStatus {
        self.updated_at = now;
        self.last_error = Some(error_detail.clone());
        let next_status = if self.attempt_count >= max_attempts {
            self.recipient_delivery = None;
            JobStatus::TerminalFailure
        } else {
            JobStatus::RetryableFailure
        };
        self.status = next_status;
        self.claim_token = None;
        self.lease_until = None;
        self.next_retry_at = (next_status == JobStatus::RetryableFailure).then(|| {
            now + chrono::Duration::seconds((5_i64 * (1_i64 << self.attempt_count.min(6))).min(300))
        });

        self.attempts.push(DeliveryAttemptRecord {
            attempt_number: self.attempt_count,
            attempted_at: now,
            outcome: format!("{next_status:?}"),
            provider_message_id: None,
            error_detail: Some(error_detail),
        });
        next_status
    }
}

#[derive(Debug, Error)]
pub enum DeliveryError {
    #[error("recipient is not in the authorized test allowlist for live Resend delivery")]
    UnauthorizedRecipient,
    #[error("transient delivery error: {0}")]
    Transient(String),
    #[error("terminal delivery error: {0}")]
    Terminal(String),
}
