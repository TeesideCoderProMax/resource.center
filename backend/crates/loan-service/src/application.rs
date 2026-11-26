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
    compute_due_at, deterministic_event_id, ensure_can_reserve, ensure_can_subscribe, ensure_staff,
    ensure_student_or_staff, validate_create_model_input, validate_new_unit_input,
    validate_unit_status_target, AuthenticatedActor, CreateDeviceModelInput, DeviceModelSummary,
    DeviceUnitSummary, DomainError, LockedModel, NewDeviceUnitInput, Reservation, Role,
    StaffInventoryModel,
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
    pub due_at: DateTime<Utc>,
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

/// Rows claimed for publishing; `unreadable` counts rows already marked FAILED because
/// their payload no longer parses.
#[derive(Debug, Clone, Default)]
pub struct ClaimedBatch {
    pub items: Vec<ClaimedOutboxItem>,
    pub unreadable: usize,
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

#[derive(Debug, Error)]
pub enum AuthVerifyError {
    #[error("Auth0 issuer/audience configuration is not set on this environment")]
    NotConfigured,
    #[error("missing or malformed Authorization Bearer token")]
    MissingBearerToken,
    /// Detail is for logs only; clients get a generic message.
    #[error("invalid access token: {0}")]
    InvalidToken(String),
    #[error("token signing keys are unavailable: {0}")]
    KeysUnavailable(String),
}

/// Port for access-token verification; the Auth0 adapter lives in infrastructure.
pub trait AccessTokenVerifierPort: Send + Sync {
    fn is_configured(&self) -> bool;
    fn verify_bearer<'a>(
        &'a self,
        auth_header: Option<&'a str>,
    ) -> BoxFuture<'a, Result<AuthenticatedActor, AuthVerifyError>>;
}

/// Port for Loan persistence and atomic outbox commits.
pub trait LoanRepositoryPort: Send + Sync {
    fn check_readiness(&self) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn list_catalogue(&self) -> BoxFuture<'_, Result<Vec<DeviceModelSummary>, ApplicationError>>;
    fn list_reservations(
        &self,
        actor: AuthenticatedActor,
    ) -> BoxFuture<'_, Result<Vec<Reservation>, ApplicationError>>;
    fn reserve_device(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
        loan_duration_days: i32,
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
    fn list_staff_inventory(
        &self,
        actor: AuthenticatedActor,
    ) -> BoxFuture<'_, Result<Vec<StaffInventoryModel>, ApplicationError>>;
    fn create_device_model(
        &self,
        actor: AuthenticatedActor,
        input: CreateDeviceModelInput,
    ) -> BoxFuture<'_, Result<StaffInventoryModel, ApplicationError>>;
    fn delete_device_model(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn add_device_unit(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
        input: NewDeviceUnitInput,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<DeviceUnitSummary, ApplicationError>>;
    fn set_unit_status(
        &self,
        actor: AuthenticatedActor,
        unit_id: String,
        status: String,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<DeviceUnitSummary, ApplicationError>>;
    fn delete_device_unit(
        &self,
        actor: AuthenticatedActor,
        unit_id: String,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
}

/// Port for the Loan-owned outbox publisher job.
pub trait OutboxStorePort: Send + Sync {
    fn claim_pending_batch(
        &self,
        batch_size: i64,
        lease_seconds: i64,
    ) -> BoxFuture<'_, Result<ClaimedBatch, ApplicationError>>;
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

/// Helper to construct availability notifications for waiting subscribers.
pub fn build_availability_outbox_events(
    transition_key: &str,
    waiting_subscriber_emails: &[String],
    device_model: &str,
    occurred_at: DateTime<Utc>,
    correlation_id: &str,
) -> Vec<NotificationRequestV1> {
    let recipients: std::collections::BTreeSet<_> = waiting_subscriber_emails.iter().collect();
    recipients
        .into_iter()
        .map(|subscriber| NotificationRequestV1 {
            event_id: deterministic_event_id(transition_key, subscriber),
            event_type: EVENT_TYPE_AVAILABILITY_REQUESTED.to_string(),
            version: CONTRACT_VERSION_V1,
            occurred_at,
            correlation_id: correlation_id.to_string(),
            recipient: subscriber.clone(),
            device_model: device_model.to_string(),
            synthetic_probe: None,
        })
        .collect()
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
    events.extend(build_availability_outbox_events(
        &format!("availability:{reservation_id}"),
        waiting_subscriber_emails,
        device_model,
        occurred_at,
        correlation_id,
    ));
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
    let batch_size = batch_size.clamp(1, 100);
    // Sequential sends each have a 35s deadline. Cover the entire batch so
    // later rows are not reclaimed while this publisher still owns the batch.
    let lease_seconds = lease_seconds.clamp(60, 3600).max(batch_size * 35 + 60);
    let claimed = store.claim_pending_batch(batch_size, lease_seconds).await?;
    let claimed_count = claimed.items.len() + claimed.unreadable;
    let mut published_count = 0usize;
    let mut failed_count = claimed.unreadable;

    for item in claimed.items {
        let publication = tokio::time::timeout(
            std::time::Duration::from_secs(35),
            publisher.publish_request(item.payload.clone()),
        )
        .await
        .unwrap_or_else(|_| {
            Err(ApplicationError::Broker(
                "publish timeout; acknowledgement unknown".into(),
            ))
        });
        match publication {
            Ok(()) => {
                if let Err(err) = store.mark_published(item.event_id.clone()).await {
                    // Sent but not marked: keep the lease so it is not resent at once; it is
                    // republished after expiry and Notifications drops the duplicate.
                    failed_count += 1;
                    tracing::warn!(
                        event_id = %item.event_id,
                        correlation_id = %item.correlation_id,
                        error = %err,
                        "published but not marked; row will be republished after lease expiry"
                    );
                    continue;
                }
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

pub fn validate_staff_action(actor: &AuthenticatedActor) -> Result<(), DomainError> {
    ensure_staff(actor)
}

/// One application-owned atomic scope. The adapter commits only an Ok result;
/// errors, cancellation and panic must roll back all writes in the scope.
pub type TransactionAction<T> = Box<
    dyn for<'tx> FnOnce(
            &'tx mut dyn LoanTransactionPort,
        ) -> BoxFuture<'tx, Result<T, ApplicationError>>
        + Send,
>;

pub trait LoanStorePort: Send + Sync {
    fn readiness(&self) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn catalogue(&self) -> BoxFuture<'_, Result<Vec<DeviceModelSummary>, ApplicationError>>;
    fn staff_inventory(&self) -> BoxFuture<'_, Result<Vec<StaffInventoryModel>, ApplicationError>>;
    /// None is staff scope; Some(subject) MUST filter in storage, before returning rows.
    fn reservations(
        &self,
        subject: Option<String>,
    ) -> BoxFuture<'_, Result<Vec<Reservation>, ApplicationError>>;
    fn transact<T: Send + 'static>(
        &self,
        action: TransactionAction<T>,
    ) -> BoxFuture<'_, Result<T, ApplicationError>>;
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_micros(Utc::now().timestamp_micros())
            .expect("current UTC timestamp")
    }
    fn new_id(&self) -> Uuid {
        Uuid::new_v4()
    }
}

/// Storage operations only. Lock order: model -> reservation/unit -> subscriptions -> outbox.
/// lock_reservation must lock its immutable model first, then reload the reservation.
pub trait LoanTransactionPort: Send {
    fn lock_model(&mut self, id: String) -> BoxFuture<'_, Result<LockedModel, ApplicationError>>;
    fn available_unit(
        &mut self,
        model: String,
    ) -> BoxFuture<'_, Result<Option<String>, ApplicationError>>;
    fn lock_reservation(
        &mut self,
        id: Uuid,
    ) -> BoxFuture<'_, Result<(LockedModel, Reservation), ApplicationError>>;
    fn insert_reservation(
        &mut self,
        reservation: Reservation,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn upsert_subscription(
        &mut self,
        actor: AuthenticatedActor,
        receipt: SubscriptionReceipt,
    ) -> BoxFuture<'_, Result<SubscriptionReceipt, ApplicationError>>;
    fn save_collection(
        &mut self,
        reservation: Reservation,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn active_subscribers(
        &mut self,
        model: String,
    ) -> BoxFuture<'_, Result<Vec<String>, ApplicationError>>;
    fn save_return(
        &mut self,
        reservation: Reservation,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn append_outbox(
        &mut self,
        events: Vec<NotificationRequestV1>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn insert_model_with_units(
        &mut self,
        input: CreateDeviceModelInput,
        now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<StaffInventoryModel, ApplicationError>>;
    fn delete_model(&mut self, model_id: String) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn insert_unit(
        &mut self,
        model_id: String,
        input: NewDeviceUnitInput,
        now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<DeviceUnitSummary, ApplicationError>>;
    fn lock_unit(
        &mut self,
        unit_id: String,
    ) -> BoxFuture<'_, Result<(LockedModel, DeviceUnitSummary), ApplicationError>>;
    fn update_unit_status(
        &mut self,
        model_id: String,
        unit_id: String,
        from_status: String,
        to_status: String,
        now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn mark_subscribers_notified(
        &mut self,
        model_id: String,
        now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
    fn delete_unit(
        &mut self,
        model_id: String,
        unit_id: String,
        was_available: bool,
        now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>>;
}

// Both PostgreSQL and the transactional fake run these exact use cases.
impl<S: LoanStorePort> LoanRepositoryPort for S {
    fn check_readiness(&self) -> BoxFuture<'_, Result<(), ApplicationError>> {
        self.readiness()
    }
    fn list_catalogue(&self) -> BoxFuture<'_, Result<Vec<DeviceModelSummary>, ApplicationError>> {
        self.catalogue()
    }
    fn list_reservations(
        &self,
        actor: AuthenticatedActor,
    ) -> BoxFuture<'_, Result<Vec<Reservation>, ApplicationError>> {
        Box::pin(async move {
            ensure_student_or_staff(&actor)?;
            let subject = if actor.has_role(&Role::Staff) {
                None
            } else {
                Some(actor.subject)
            };
            self.reservations(subject).await
        })
    }
    fn reserve_device(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
        loan_duration_days: i32,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<ReservationReceipt, ApplicationError>> {
        Box::pin(async move {
            validate_reserve_request(&actor, &model_id)?;
            let now = self.now();
            let id = self.new_id();
            self.transact(Box::new(move |tx| {
                Box::pin(async move {
                    let model = tx.lock_model(model_id.clone()).await?;
                    let due_at = compute_due_at(now, loan_duration_days, model.loan_duration_days)?;
                    ensure_can_reserve(&model)?;
                    let unit_id = tx
                        .available_unit(model_id.clone())
                        .await?
                        .ok_or_else(|| DomainError::NoAvailableUnits(model_id.clone()))?;
                    tx.insert_reservation(Reservation {
                        reservation_id: id,
                        model_id: model_id.clone(),
                        unit_id: unit_id.clone(),
                        student_sub: actor.subject,
                        student_email: actor.email,
                        status: "RESERVED".into(),
                        reserved_at: now,
                        due_at,
                        collected_at: None,
                        returned_at: None,
                        correlation_id: correlation_id.clone(),
                    })
                    .await?;
                    Ok(ReservationReceipt {
                        reservation_id: id,
                        model_id,
                        unit_id,
                        status: "RESERVED".into(),
                        reserved_at: now,
                        due_at,
                        correlation_id,
                    })
                })
            }))
            .await
        })
    }
    fn subscribe_availability(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
    ) -> BoxFuture<'_, Result<SubscriptionReceipt, ApplicationError>> {
        Box::pin(async move {
            validate_reserve_request(&actor, &model_id)?;
            let receipt = SubscriptionReceipt {
                subscription_id: self.new_id(),
                model_id: model_id.clone(),
                status: "ACTIVE".into(),
                created_at: self.now(),
            };
            self.transact(Box::new(move |tx| {
                Box::pin(async move {
                    let model = tx.lock_model(model_id).await?;
                    ensure_can_subscribe(&model)?;
                    tx.upsert_subscription(actor, receipt).await
                })
            }))
            .await
        })
    }
    fn record_collection(
        &self,
        actor: AuthenticatedActor,
        reservation_id: Uuid,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<CollectionReceipt, ApplicationError>> {
        Box::pin(async move {
            ensure_staff(&actor)?;
            let now = self.now();
            self.transact(Box::new(move |tx| {
                Box::pin(async move {
                    let (model, mut reservation) = tx.lock_reservation(reservation_id).await?;
                    reservation.collect(now)?;
                    let event = build_collection_outbox_event(
                        reservation_id,
                        &reservation.student_email,
                        &model.name,
                        now,
                        &correlation_id,
                    );
                    let receipt = CollectionReceipt {
                        reservation_id,
                        unit_id: reservation.unit_id.clone(),
                        status: reservation.status.clone(),
                        collected_at: now,
                        due_at: reservation.due_at,
                        outbox_event_id: event.event_id.clone(),
                    };
                    tx.save_collection(reservation).await?;
                    tx.append_outbox(vec![event]).await?;
                    Ok(receipt)
                })
            }))
            .await
        })
    }
    fn record_return(
        &self,
        actor: AuthenticatedActor,
        reservation_id: Uuid,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<ReturnReceipt, ApplicationError>> {
        Box::pin(async move {
            ensure_staff(&actor)?;
            let now = self.now();
            self.transact(Box::new(move |tx| {
                Box::pin(async move {
                    let (model, mut reservation) = tx.lock_reservation(reservation_id).await?;
                    reservation.return_device(now)?;
                    let subscribers = tx.active_subscribers(model.id).await?;
                    let events = build_return_outbox_events(
                        reservation_id,
                        &reservation.student_email,
                        &subscribers,
                        &model.name,
                        now,
                        &correlation_id,
                    );
                    let receipt = ReturnReceipt {
                        reservation_id,
                        model_id: reservation.model_id.clone(),
                        unit_id: reservation.unit_id.clone(),
                        status: reservation.status.clone(),
                        returned_at: now,
                        outbox_event_ids: events.iter().map(|e| e.event_id.clone()).collect(),
                    };
                    tx.save_return(reservation).await?;
                    tx.append_outbox(events).await?;
                    Ok(receipt)
                })
            }))
            .await
        })
    }
    fn list_staff_inventory(
        &self,
        actor: AuthenticatedActor,
    ) -> BoxFuture<'_, Result<Vec<StaffInventoryModel>, ApplicationError>> {
        Box::pin(async move {
            ensure_staff(&actor)?;
            self.staff_inventory().await
        })
    }
    fn create_device_model(
        &self,
        actor: AuthenticatedActor,
        input: CreateDeviceModelInput,
    ) -> BoxFuture<'_, Result<StaffInventoryModel, ApplicationError>> {
        Box::pin(async move {
            ensure_staff(&actor)?;
            let validated = validate_create_model_input(input)?;
            let now = self.now();
            self.transact(Box::new(move |tx| {
                Box::pin(async move { tx.insert_model_with_units(validated, now).await })
            }))
            .await
        })
    }
    fn delete_device_model(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            ensure_staff(&actor)?;
            let model_id = model_id.trim().to_string();
            if model_id.is_empty() {
                return Err(DomainError::InvalidInput("modelId must not be empty").into());
            }
            self.transact(Box::new(move |tx| {
                Box::pin(async move {
                    tx.lock_model(model_id.clone()).await?;
                    tx.delete_model(model_id).await
                })
            }))
            .await
        })
    }
    fn add_device_unit(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
        input: NewDeviceUnitInput,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<DeviceUnitSummary, ApplicationError>> {
        Box::pin(async move {
            ensure_staff(&actor)?;
            let model_id = model_id.trim().to_string();
            if model_id.is_empty() {
                return Err(DomainError::InvalidInput("modelId must not be empty").into());
            }
            let validated = validate_new_unit_input(input)?;
            let now = self.now();
            let stock_key = self.new_id();
            self.transact(Box::new(move |tx| {
                Box::pin(async move {
                    let model = tx.lock_model(model_id.clone()).await?;
                    let unit = tx.insert_unit(model.id.clone(), validated, now).await?;
                    if unit.status == "AVAILABLE" && model.available_units == 0 {
                        let subscribers = tx.active_subscribers(model.id.clone()).await?;
                        if !subscribers.is_empty() {
                            tx.mark_subscribers_notified(model.id.clone(), now).await?;
                            let events = build_availability_outbox_events(
                                &format!("availability:stock:{stock_key}"),
                                &subscribers,
                                &model.name,
                                now,
                                &correlation_id,
                            );
                            tx.append_outbox(events).await?;
                        }
                    }
                    Ok(unit)
                })
            }))
            .await
        })
    }
    fn set_unit_status(
        &self,
        actor: AuthenticatedActor,
        unit_id: String,
        status: String,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<DeviceUnitSummary, ApplicationError>> {
        Box::pin(async move {
            ensure_staff(&actor)?;
            let unit_id = unit_id.trim().to_string();
            if unit_id.is_empty() {
                return Err(DomainError::InvalidInput("unitId must not be empty").into());
            }
            let target = validate_unit_status_target(&status)?;
            let now = self.now();
            let stock_key = self.new_id();
            self.transact(Box::new(move |tx| {
                Box::pin(async move {
                    let (model, unit) = tx.lock_unit(unit_id.clone()).await?;
                    if unit.status == "RESERVED" || unit.status == "COLLECTED" {
                        return Err(DomainError::InvalidTransition {
                            from: unit.status,
                            to: target,
                        }
                        .into());
                    }
                    if unit.status == target {
                        return Ok(unit);
                    }
                    tx.update_unit_status(
                        model.id.clone(),
                        unit.unit_id.clone(),
                        unit.status.clone(),
                        target.clone(),
                        now,
                    )
                    .await?;
                    if unit.status == "MAINTENANCE"
                        && target == "AVAILABLE"
                        && model.available_units == 0
                    {
                        let subscribers = tx.active_subscribers(model.id.clone()).await?;
                        if !subscribers.is_empty() {
                            tx.mark_subscribers_notified(model.id.clone(), now).await?;
                            let events = build_availability_outbox_events(
                                &format!("availability:stock:{stock_key}"),
                                &subscribers,
                                &model.name,
                                now,
                                &correlation_id,
                            );
                            tx.append_outbox(events).await?;
                        }
                    }
                    Ok(DeviceUnitSummary {
                        status: target,
                        ..unit
                    })
                })
            }))
            .await
        })
    }
    fn delete_device_unit(
        &self,
        actor: AuthenticatedActor,
        unit_id: String,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            ensure_staff(&actor)?;
            let unit_id = unit_id.trim().to_string();
            if unit_id.is_empty() {
                return Err(DomainError::InvalidInput("unitId must not be empty").into());
            }
            let now = self.now();
            self.transact(Box::new(move |tx| {
                Box::pin(async move {
                    let (model, unit) = tx.lock_unit(unit_id).await?;
                    if unit.status == "RESERVED" || unit.status == "COLLECTED" {
                        return Err(DomainError::ResourceInUse(
                            "cannot remove a unit that is currently reserved or out on loan".into(),
                        )
                        .into());
                    }
                    tx.delete_unit(model.id, unit.unit_id, unit.status == "AVAILABLE", now)
                        .await
                })
            }))
            .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct FakeOutbox {
        items: Vec<ClaimedOutboxItem>,
        published: Mutex<Vec<String>>,
        failed: Mutex<Vec<(String, String)>>,
        fail_mark_published: Option<String>,
        unreadable: usize,
    }

    impl OutboxStorePort for FakeOutbox {
        fn claim_pending_batch(
            &self,
            _batch_size: i64,
            _lease_seconds: i64,
        ) -> BoxFuture<'_, Result<ClaimedBatch, ApplicationError>> {
            let batch = ClaimedBatch {
                items: self.items.clone(),
                unreadable: self.unreadable,
            };
            Box::pin(async move { Ok(batch) })
        }

        fn mark_published(&self, event_id: String) -> BoxFuture<'_, Result<(), ApplicationError>> {
            if self.fail_mark_published.as_ref() == Some(&event_id) {
                return Box::pin(
                    async move { Err(ApplicationError::Persistence("db down".into())) },
                );
            }
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
            fail_mark_published: None,
            unreadable: 0,
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

    #[tokio::test]
    async fn mark_published_error_does_not_abort_batch() {
        let events = build_return_outbox_events(
            Uuid::nil(),
            "borrower@tees.ac.uk",
            &["waiter@tees.ac.uk".to_string()],
            "Dell Latitude 7440",
            Utc::now(),
            "corr-1",
        );
        let first = events[0].event_id.clone();
        let second = events[1].event_id.clone();
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
            fail_mark_published: Some(first),
            unreadable: 0,
        };
        let summary = run_outbox_publish_batch(&store, &FakeBroker { should_fail: false }, 10, 60)
            .await
            .unwrap();
        assert_eq!(summary.claimed_count, 2);
        assert_eq!(summary.published_count, 1);
        assert_eq!(summary.failed_count, 1);
        assert_eq!(*store.published.lock().unwrap(), vec![second]);
        assert!(store.failed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unreadable_rows_count_as_failed() {
        let store = FakeOutbox {
            items: Vec::new(),
            published: Mutex::new(Vec::new()),
            failed: Mutex::new(Vec::new()),
            fail_mark_published: None,
            unreadable: 2,
        };
        let summary = run_outbox_publish_batch(&store, &FakeBroker { should_fail: false }, 10, 60)
            .await
            .unwrap();
        assert_eq!(summary.claimed_count, 2);
        assert_eq!(summary.failed_count, 2);
    }
}
