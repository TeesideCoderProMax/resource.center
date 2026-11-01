use std::future::Future;
use std::pin::Pin;

use chrono::{DateTime, Utc};
use rc_shared::{
    NotificationRequestV1, CONTRACT_VERSION_V1, EVENT_TYPE_AVAILABILITY_REQUESTED,
    EVENT_TYPE_COLLECTION_RECORDED, EVENT_TYPE_RETURN_RECORDED,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::domain::{
    compute_two_day_due_at, deterministic_event_id, ensure_staff, ensure_student_or_staff,
    AuthenticatedActor, DeviceModelSummary, DomainError,
};

#[derive(Debug, Error)]
pub enum ApplicationError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error("resource not found: {0}")]
    NotFound(String),
    #[error("persistence error: {0}")]
    Persistence(String),
    #[error("broker error: {0}")]
    Broker(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservationReceipt {
    pub reservation_id: Uuid,
    pub model_id: String,
    pub unit_id: String,
    pub status: String,
    pub reserved_at: DateTime<Utc>,
    pub correlation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionReceipt {
    pub subscription_id: Uuid,
    pub model_id: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionReceipt {
    pub reservation_id: Uuid,
    pub unit_id: String,
    pub status: String,
    pub collected_at: DateTime<Utc>,
    pub due_at: DateTime<Utc>,
    pub outbox_event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnReceipt {
    pub reservation_id: Uuid,
    pub unit_id: String,
    pub model_id: String,
    pub status: String,
    pub returned_at: DateTime<Utc>,
    pub outbox_event_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ClaimedOutboxItem {
    pub event_id: String,
    pub correlation_id: String,
    pub attempt_count: i32,
    pub payload: NotificationRequestV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishBatchSummary {
    pub claimed_count: usize,
    pub published_count: usize,
    pub failed_count: usize,
    pub duration_ms: u128,
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Port for Loan persistence and atomic outbox commits.
pub trait LoanRepositoryPort: Send + Sync {
    fn check_readiness(&self) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn list_catalogue(&self) -> BoxFuture<'_, Result<Vec<DeviceModelSummary>, ApplicationError>>;
    fn reserve_device(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<ReservationReceipt, ApplicationError>>;
    fn subscribe_availability(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
    ) -> BoxFuture<'_, Result<SubscriptionReceipt, ApplicationError>>;
    fn record_collection(
        &self,
        actor: AuthenticatedActor,
        reservation_id: Uuid,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<CollectionReceipt, ApplicationError>>;
    fn record_return(
        &self,
        actor: AuthenticatedActor,
        reservation_id: Uuid,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<ReturnReceipt, ApplicationError>>;
}

/// Port for the Loan-owned outbox publisher job.
pub trait OutboxStorePort: Send + Sync {
    fn claim_pending_batch(
        &self,
        batch_size: i64,
        lease_seconds: i64,
    ) -> BoxFuture<'_, Result<Vec<ClaimedOutboxItem>, ApplicationError>>;
    fn mark_published(&self, event_id: String) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn record_publish_failure(
        &self,
        event_id: String,
        error_message: String,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
}

/// Port for sending notification requests to the durable queue (Azure Service Bus).
pub trait QueuePublisherPort: Send + Sync {
    fn publish_request(
        &self,
        request: NotificationRequestV1,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
}

/// Helper to construct the outbox events produced when staff records a collection.
pub fn build_collection_outbox_event(
    reservation_id: Uuid,
    recipient: &str,
    device_model: &str,
    occurred_at: DateTime<Utc>,
    correlation_id: &str,
) -> NotificationRequestV1 {
    let event_id = deterministic_event_id(&format!("collect:{reservation_id}"), recipient);
    NotificationRequestV1 {
        event_id,
        event_type: EVENT_TYPE_COLLECTION_RECORDED.to_string(),
        version: CONTRACT_VERSION_V1,
        occurred_at,
        correlation_id: correlation_id.to_string(),
        recipient: recipient.to_string(),
        device_model: device_model.to_string(),
        synthetic_probe: None,
    }
}

/// Helper to construct the return confirmation outbox event plus availability notifications
/// for eligible subscribed students in the same atomic PostgreSQL transaction.
pub fn build_return_outbox_events(
    reservation_id: Uuid,
    borrower_email: &str,
    waiting_subscriber_emails: &[String],
    device_model: &str,
    occurred_at: DateTime<Utc>,
    correlation_id: &str,
) -> Vec<NotificationRequestV1> {
    let mut events = Vec::with_capacity(1 + waiting_subscriber_emails.len());
    events.push(NotificationRequestV1 {
        event_id: deterministic_event_id(&format!("return:{reservation_id}"), borrower_email),
        event_type: EVENT_TYPE_RETURN_RECORDED.to_string(),
        version: CONTRACT_VERSION_V1,
        occurred_at,
        correlation_id: correlation_id.to_string(),
        recipient: borrower_email.to_string(),
        device_model: device_model.to_string(),
        synthetic_probe: None,
    });
    for subscriber in waiting_subscriber_emails {
        events.push(NotificationRequestV1 {
            event_id: deterministic_event_id(&format!("availability:{reservation_id}"), subscriber),
            event_type: EVENT_TYPE_AVAILABILITY_REQUESTED.to_string(),
            version: CONTRACT_VERSION_V1,
            occurred_at,
            correlation_id: correlation_id.to_string(),
            recipient: subscriber.clone(),
            device_model: device_model.to_string(),
            synthetic_probe: None,
        });
    }
    events
}

/// Executes a single finite outbox publishing batch:
/// 1. Claims committed `PENDING` rows with an expiry lease (`FOR UPDATE SKIP LOCKED`).
/// 2. Publishes each message to Service Bus.
/// 3. Marks `PUBLISHED` only after broker acknowledgement; records `last_error` on failure
///    so failed items remain recoverable on subsequent runs.
pub async fn run_outbox_publish_batch<S, P>(
    store: &S,
    publisher: &P,
    batch_size: i64,
    lease_seconds: i64,
) -> Result<PublishBatchSummary, ApplicationError>
where
    S: OutboxStorePort,
    P: QueuePublisherPort,
{
    let started = std::time::Instant::now();
    let claimed = store.claim_pending_batch(batch_size, lease_seconds).await?;
    let claimed_count = claimed.len();
    let mut published_count = 0usize;
    let mut failed_count = 0usize;

    for item in claimed {
        match publisher.publish_request(item.payload.clone()).await {
            Ok(()) => {
                store.mark_published(item.event_id.clone()).await?;
                published_count += 1;
                tracing::info!(
                    event_id = %item.event_id,
                    correlation_id = %item.correlation_id,
                    attempt_count = item.attempt_count,
                    recipient = %item.payload.redacted_recipient(),
                    "outbox event published and acknowledged by broker"
                );
            }
            Err(err) => {
                failed_count += 1;
                let msg = err.to_string();
                tracing::warn!(
                    event_id = %item.event_id,
                    correlation_id = %item.correlation_id,
                    attempt_count = item.attempt_count,
                    error = %msg,
                    "broker publication failed; leaving outbox row recoverable"
                );
                let _ = store.record_publish_failure(item.event_id, msg).await;
            }
        }
    }

    Ok(PublishBatchSummary {
        claimed_count,
        published_count,
        failed_count,
        duration_ms: started.elapsed().as_millis(),
    })
}

pub fn validate_reserve_request(
    actor: &AuthenticatedActor,
    model_id: &str,
) -> Result<(), DomainError> {
    ensure_student_or_staff(actor)?;
    if model_id.trim().is_empty() {
        return Err(DomainError::InvalidInput("modelId must not be empty"));
    }
    Ok(())
}

pub fn validate_staff_action(actor: &AuthenticatedActor) -> Result<DateTime<Utc>, DomainError> {
    ensure_staff(actor)?;
    Ok(compute_two_day_due_at(Utc::now()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeOutbox {
        items: Vec<ClaimedOutboxItem>,
        published: Mutex<Vec<String>>,
        failed: Mutex<Vec<(String, String)>>,
    }

    impl OutboxStorePort for FakeOutbox {
        fn claim_pending_batch(
            &self,
            _batch_size: i64,
            _lease_seconds: i64,
        ) -> BoxFuture<'_, Result<Vec<ClaimedOutboxItem>, ApplicationError>> {
            let items = self.items.clone();
            Box::pin(async move { Ok(items) })
        }

        fn mark_published(&self, event_id: String) -> BoxFuture<'_, Result<(), ApplicationError>> {
            self.published.lock().unwrap().push(event_id);
            Box::pin(async move { Ok(()) })
        }

        fn record_publish_failure(
            &self,
            event_id: String,
            error_message: String,
        ) -> BoxFuture<'_, Result<(), ApplicationError>> {
            self.failed.lock().unwrap().push((event_id, error_message));
            Box::pin(async move { Ok(()) })
        }
    }

    struct FakeBroker {
        should_fail: bool,
    }

    impl QueuePublisherPort for FakeBroker {
        fn publish_request(
            &self,
            _request: NotificationRequestV1,
        ) -> BoxFuture<'_, Result<(), ApplicationError>> {
            let fail = self.should_fail;
            Box::pin(async move {
                if fail {
                    Err(ApplicationError::Broker("simulated broker outage".into()))
                } else {
                    Ok(())
                }
            })
        }
    }

    #[tokio::test]
    async fn outbox_batch_marks_published_only_after_broker_ack() {
        let events = build_return_outbox_events(
            Uuid::nil(),
            "borrower@tees.ac.uk",
            &["waiter@tees.ac.uk".to_string()],
            "Dell Latitude 7440",
            Utc::now(),
            "corr-1",
        );
        assert_eq!(events.len(), 2);

        let store = FakeOutbox {
            items: events
                .into_iter()
                .map(|p| ClaimedOutboxItem {
                    event_id: p.event_id.clone(),
                    correlation_id: p.correlation_id.clone(),
                    attempt_count: 1,
                    payload: p,
                })
                .collect(),
            published: Mutex::new(Vec::new()),
            failed: Mutex::new(Vec::new()),
        };

        let summary_fail =
            run_outbox_publish_batch(&store, &FakeBroker { should_fail: true }, 10, 60)
                .await
                .unwrap();
        assert_eq!(summary_fail.published_count, 0);
        assert_eq!(summary_fail.failed_count, 2);
        assert!(store.published.lock().unwrap().is_empty());
        assert_eq!(store.failed.lock().unwrap().len(), 2);

        let summary_ok =
            run_outbox_publish_batch(&store, &FakeBroker { should_fail: false }, 10, 60)
                .await
                .unwrap();
        assert_eq!(summary_ok.published_count, 2);
        assert_eq!(store.published.lock().unwrap().len(), 2);
    }
}
