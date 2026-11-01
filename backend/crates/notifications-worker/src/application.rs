use crate::domain::{DeliveryError, JobStatus, NotificationJobDocument};
use async_trait::async_trait;
use chrono::Utc;
use rc_shared::{redact_email, NotificationRequestV1};
use serde::Serialize;
use thiserror::Error;
use tracing::{info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionedJobDocument {
    pub document: NotificationJobDocument,
    pub etag: String,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateJobOutcome {
    Created(VersionedJobDocument),
    AlreadyExists,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceJobOutcome {
    Replaced(VersionedJobDocument),
    PreconditionFailed,
}

#[derive(Debug, Error)]
pub enum JobStoreError {
    #[error("job document not found: {0}")]
    NotFound(String),
    #[error("cosmos store error: {0}")]
    Storage(String),
}

#[allow(
    clippy::double_must_use,
    reason = "async-trait adds must_use to methods returning must-use futures"
)]
#[async_trait]
pub trait NotificationJobStorePort: Send + Sync {
    async fn create_job(
        &self,
        doc: &NotificationJobDocument,
    ) -> Result<CreateJobOutcome, JobStoreError>;

    async fn get_job(&self, event_id: &str) -> Result<Option<VersionedJobDocument>, JobStoreError>;

    async fn replace_job(
        &self,
        doc: &NotificationJobDocument,
        if_match_etag: &str,
    ) -> Result<ReplaceJobOutcome, JobStoreError>;

    async fn delete_job(&self, event_id: &str) -> Result<(), JobStoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailDeliveryResult {
    pub provider_message_id: String,
    pub delivery_mode: String,
}

#[allow(
    clippy::double_must_use,
    reason = "async-trait adds must_use to methods returning must-use futures"
)]
#[async_trait]
pub trait EmailDeliveryPort: Send + Sync {
    fn configured_mode(&self) -> &str;

    async fn send_notification(
        &self,
        request: &NotificationRequestV1,
    ) -> Result<EmailDeliveryResult, DeliveryError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MessageDisposition {
    /// Complete/settle the message on the queue (`DELETE` lock URI).
    Complete {
        event_id: String,
        duplicate_skipped: bool,
        final_status: JobStatus,
    },
    /// Keep the broker lock and retry locally; durable timing remains in Cosmos.
    Wait { event_id: String, seconds: u64 },
    /// Abandon/unlock the message on the queue (`PUT` lock URI) for retry or because another worker holds the ETag.
    Abandon { event_id: String, reason: String },
}

pub struct NotificationProcessor<S, E> {
    store: S,
    email_sender: E,
    max_attempts: u32,
}

impl<S, E> NotificationProcessor<S, E>
where
    S: NotificationJobStorePort,
    E: EmailDeliveryPort,
{
    pub fn new(store: S, email_sender: E, max_attempts: u32) -> Self {
        Self {
            store,
            email_sender,
            max_attempts: max_attempts.clamp(1, 10),
        }
    }

    /// Retry transient store failures and durable waits on the same broker delivery.
    pub async fn process_until_settled(&self, body: &str) -> MessageDisposition {
        loop {
            match self.process_raw_message(body).await {
                Ok(MessageDisposition::Wait { seconds, .. }) => {
                    tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
                }
                Ok(outcome) => return outcome,
                Err(_) => {
                    warn!("job store unavailable; holding broker lock before retry");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    pub async fn process_raw_message(
        &self,
        raw_body: &str,
    ) -> Result<MessageDisposition, JobStoreError> {
        let req: NotificationRequestV1 = match serde_json::from_str(raw_body) {
            Ok(parsed) => parsed,
            Err(_) => {
                warn!("received malformed notification message payload; abandoning for dead-letter exhaustion");
                return Ok(MessageDisposition::Abandon {
                    event_id: "invalid-json".to_string(),
                    reason: "malformed JSON payload".to_string(),
                });
            }
        };

        if let Err(err) = req.validate() {
            warn!(
                event_id = %req.event_id,
                correlation_id = %req.correlation_id,
                error = %err,
                "notification contract validation failed; abandoning for dead-letter exhaustion"
            );
            return Ok(MessageDisposition::Abandon {
                event_id: req.event_id,
                reason: format!("contract validation failed: {err}"),
            });
        }

        self.process_request(&req).await
    }

    pub async fn process_request(
        &self,
        req: &NotificationRequestV1,
    ) -> Result<MessageDisposition, JobStoreError> {
        let now = Utc::now();
        let is_synthetic = req.synthetic_probe.unwrap_or(false);
        let initial_mode = if is_synthetic {
            "fake"
        } else {
            self.email_sender.configured_mode()
        };

        let candidate = NotificationJobDocument::new_claimed(req, initial_mode, now);

        let claimed_versioned = match self.store.create_job(&candidate).await? {
            CreateJobOutcome::Created(versioned) => {
                info!(
                    event_id = %req.event_id,
                    correlation_id = %req.correlation_id,
                    recipient = %redact_email(&req.recipient),
                    synthetic_probe = is_synthetic,
                    "claimed new notification job in Cosmos DB"
                );
                versioned
            }
            CreateJobOutcome::AlreadyExists => {
                let existing = self
                    .store
                    .get_job(&req.event_id)
                    .await?
                    .ok_or_else(|| JobStoreError::NotFound(req.event_id.clone()))?;

                if existing.document.is_terminal() {
                    info!(
                        event_id = %req.event_id,
                        correlation_id = %req.correlation_id,
                        status = ?existing.document.status,
                        "duplicate queue delivery detected for terminal Cosmos job; completing without re-sending email"
                    );
                    return Ok(MessageDisposition::Complete {
                        event_id: req.event_id.clone(),
                        duplicate_skipped: true,
                        final_status: existing.document.status,
                    });
                }

                let available = existing.document.available_at();
                if available > Utc::now() {
                    return Ok(MessageDisposition::Wait {
                        event_id: req.event_id.clone(),
                        seconds: (available - Utc::now()).num_seconds().clamp(1, 10) as u64,
                    });
                }
                if existing.document.attempt_count >= self.max_attempts {
                    let mut exhausted = existing.document;
                    exhausted.mark_failure(
                        "attempt budget exhausted after lease expiry".into(),
                        self.max_attempts,
                        Utc::now(),
                    );
                    return match self.store.replace_job(&exhausted, &existing.etag).await? {
                        ReplaceJobOutcome::Replaced(_) => Ok(MessageDisposition::Complete {
                            event_id: req.event_id.clone(),
                            duplicate_skipped: false,
                            final_status: JobStatus::TerminalFailure,
                        }),
                        ReplaceJobOutcome::PreconditionFailed => Ok(MessageDisposition::Wait {
                            event_id: req.event_id.clone(),
                            seconds: 1,
                        }),
                    };
                }
                let mut reclaimed = existing.document;
                reclaimed.mark_reclaimed(Utc::now());
                match self.store.replace_job(&reclaimed, &existing.etag).await? {
                    ReplaceJobOutcome::Replaced(versioned) => {
                        info!(
                            event_id = %req.event_id,
                            correlation_id = %req.correlation_id,
                            attempt_count = versioned.document.attempt_count,
                            "reclaimed non-terminal notification job via optimistic ETag precondition"
                        );
                        versioned
                    }
                    ReplaceJobOutcome::PreconditionFailed => {
                        warn!(
                            event_id = %req.event_id,
                            correlation_id = %req.correlation_id,
                            "ETag precondition failed while reclaiming job; another worker updated the record"
                        );
                        return Ok(MessageDisposition::Wait {
                            event_id: req.event_id.clone(),
                            seconds: 1,
                        });
                    }
                }
            }
        };

        let mut doc = claimed_versioned.document;
        let mut current_etag = claimed_versioned.etag;
        let token = doc.claim_token.clone().unwrap_or_default();
        if !doc.owns(&token, Utc::now()) {
            return Ok(MessageDisposition::Wait {
                event_id: req.event_id.clone(),
                seconds: 1,
            });
        }
        let send = tokio::time::timeout(
            std::time::Duration::from_secs(crate::domain::EMAIL_TIMEOUT_SECONDS),
            self.email_sender.send_notification(req),
        );
        tokio::pin!(send);
        let mut renewal = tokio::time::interval(std::time::Duration::from_secs(10));
        renewal.tick().await;
        let outcome = loop {
            tokio::select! {
                result = &mut send => break result.unwrap_or_else(|_| Err(DeliveryError::Transient("email timeout; outcome unknown".into()))),
                _ = renewal.tick() => {
                    if !doc.owns(&token, Utc::now()) {
                        return Ok(MessageDisposition::Wait { event_id: req.event_id.clone(), seconds: 1 });
                    }
                    doc.lease_until = Some(Utc::now() + chrono::Duration::seconds(crate::domain::LEASE_SECONDS));
                    match self.store.replace_job(&doc, &current_etag).await? {
                        ReplaceJobOutcome::Replaced(updated) => current_etag = updated.etag,
                        ReplaceJobOutcome::PreconditionFailed => return Ok(MessageDisposition::Wait { event_id: req.event_id.clone(), seconds: 1 }),
                    }
                }
            }
        };
        if !doc.owns(&token, Utc::now()) {
            return Ok(MessageDisposition::Wait {
                event_id: req.event_id.clone(),
                seconds: 1,
            });
        }
        match outcome {
            Ok(delivery) => {
                doc.mark_delivered(
                    delivery.provider_message_id.clone(),
                    &delivery.delivery_mode,
                    Utc::now(),
                );
                match self.store.replace_job(&doc, &current_etag).await? {
                    ReplaceJobOutcome::Replaced(updated) => {
                        info!(
                            event_id = %req.event_id,
                            correlation_id = %req.correlation_id,
                            delivery_mode = %delivery.delivery_mode,
                            provider_message_id = %delivery.provider_message_id,
                            attempt_count = updated.document.attempt_count,
                            "notification delivered and recorded in Cosmos DB"
                        );
                        Ok(MessageDisposition::Complete {
                            event_id: req.event_id.clone(),
                            duplicate_skipped: false,
                            final_status: JobStatus::Delivered,
                        })
                    }
                    ReplaceJobOutcome::PreconditionFailed => {
                        warn!(
                            event_id = %req.event_id,
                            correlation_id = %req.correlation_id,
                            "ETag precondition failed when persisting Delivered state"
                        );
                        Ok(MessageDisposition::Wait {
                            event_id: req.event_id.clone(),
                            seconds: 1,
                        })
                    }
                }
            }
            Err(err) => {
                let effective_max = match err {
                    DeliveryError::Terminal(_) | DeliveryError::UnauthorizedRecipient => 1,
                    DeliveryError::Transient(_) => self.max_attempts,
                };
                let next_status = doc.mark_failure(err.to_string(), effective_max, Utc::now());
                match self.store.replace_job(&doc, &current_etag).await? {
                    ReplaceJobOutcome::Replaced(_) => {
                        if next_status == JobStatus::TerminalFailure {
                            warn!(
                                event_id = %req.event_id,
                                correlation_id = %req.correlation_id,
                                error = %err,
                                "notification failed terminally; recording TerminalFailure in Cosmos DB and completing message"
                            );
                            Ok(MessageDisposition::Complete {
                                event_id: req.event_id.clone(),
                                duplicate_skipped: false,
                                final_status: JobStatus::TerminalFailure,
                            })
                        } else {
                            warn!(
                                event_id = %req.event_id,
                                correlation_id = %req.correlation_id,
                                error = %err,
                                attempt_count = doc.attempt_count,
                                "notification failed transiently; waiting until durable retry time"
                            );
                            Ok(MessageDisposition::Wait {
                                event_id: req.event_id.clone(),
                                seconds: 10,
                            })
                        }
                    }
                    ReplaceJobOutcome::PreconditionFailed => Ok(MessageDisposition::Wait {
                        event_id: req.event_id.clone(),
                        seconds: 1,
                    }),
                }
            }
        }
    }
}

#[async_trait]
impl<S: NotificationJobStorePort + ?Sized> NotificationJobStorePort for &S {
    async fn create_job(
        &self,
        doc: &NotificationJobDocument,
    ) -> Result<CreateJobOutcome, JobStoreError> {
        (**self).create_job(doc).await
    }

    async fn get_job(&self, event_id: &str) -> Result<Option<VersionedJobDocument>, JobStoreError> {
        (**self).get_job(event_id).await
    }

    async fn replace_job(
        &self,
        doc: &NotificationJobDocument,
        if_match_etag: &str,
    ) -> Result<ReplaceJobOutcome, JobStoreError> {
        (**self).replace_job(doc, if_match_etag).await
    }

    async fn delete_job(&self, event_id: &str) -> Result<(), JobStoreError> {
        (**self).delete_job(event_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_shared::{CONTRACT_VERSION_V1, EVENT_TYPE_AVAILABILITY_REQUESTED};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct InMemoryCosmosStore {
        docs: Mutex<HashMap<String, VersionedJobDocument>>,
        etag_counter: AtomicUsize,
    }

    impl InMemoryCosmosStore {
        fn new() -> Self {
            Self {
                docs: Mutex::new(HashMap::new()),
                etag_counter: AtomicUsize::new(1),
            }
        }

        fn next_etag(&self) -> String {
            let n = self.etag_counter.fetch_add(1, Ordering::SeqCst);
            format!("\"etag-{n}\"")
        }
    }

    #[async_trait]
    impl NotificationJobStorePort for InMemoryCosmosStore {
        async fn create_job(
            &self,
            doc: &NotificationJobDocument,
        ) -> Result<CreateJobOutcome, JobStoreError> {
            let mut guard = self.docs.lock().expect("lock");
            if guard.contains_key(&doc.event_id) {
                return Ok(CreateJobOutcome::AlreadyExists);
            }
            let versioned = VersionedJobDocument {
                document: doc.clone(),
                etag: self.next_etag(),
            };
            guard.insert(doc.event_id.clone(), versioned.clone());
            Ok(CreateJobOutcome::Created(versioned))
        }

        async fn get_job(
            &self,
            event_id: &str,
        ) -> Result<Option<VersionedJobDocument>, JobStoreError> {
            let guard = self.docs.lock().expect("lock");
            Ok(guard.get(event_id).cloned())
        }

        async fn replace_job(
            &self,
            doc: &NotificationJobDocument,
            if_match_etag: &str,
        ) -> Result<ReplaceJobOutcome, JobStoreError> {
            let mut guard = self.docs.lock().expect("lock");
            let Some(existing) = guard.get(&doc.event_id) else {
                return Err(JobStoreError::NotFound(doc.event_id.clone()));
            };
            if existing.etag != if_match_etag {
                return Ok(ReplaceJobOutcome::PreconditionFailed);
            }
            let updated = VersionedJobDocument {
                document: doc.clone(),
                etag: self.next_etag(),
            };
            guard.insert(doc.event_id.clone(), updated.clone());
            Ok(ReplaceJobOutcome::Replaced(updated))
        }

        async fn delete_job(&self, event_id: &str) -> Result<(), JobStoreError> {
            let mut guard = self.docs.lock().expect("lock");
            guard.remove(event_id);
            Ok(())
        }
    }

    struct CountingFakeSender {
        calls: AtomicUsize,
        fail_first: bool,
    }

    impl CountingFakeSender {
        fn new(fail_first: bool) -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fail_first,
            }
        }
    }

    #[async_trait]
    impl EmailDeliveryPort for &CountingFakeSender {
        fn configured_mode(&self) -> &str {
            "fake"
        }

        async fn send_notification(
            &self,
            request: &NotificationRequestV1,
        ) -> Result<EmailDeliveryResult, DeliveryError> {
            let call_num = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_first && call_num == 1 {
                return Err(DeliveryError::Transient("simulated timeout".to_string()));
            }
            Ok(EmailDeliveryResult {
                provider_message_id: format!("fake-{}-{call_num}", request.event_id),
                delivery_mode: "fake".to_string(),
            })
        }
    }

    fn sample_request(event_id: &str) -> NotificationRequestV1 {
        NotificationRequestV1 {
            event_id: event_id.to_string(),
            event_type: EVENT_TYPE_AVAILABILITY_REQUESTED.to_string(),
            version: CONTRACT_VERSION_V1,
            occurred_at: Utc::now(),
            correlation_id: "corr-test-1".to_string(),
            recipient: "student@example.edu".to_string(),
            device_model: "Surface Pro 10".to_string(),
            synthetic_probe: Some(true),
        }
    }

    #[tokio::test]
    async fn deduplicates_already_delivered_event_without_resending_email() {
        let store = InMemoryCosmosStore::new();
        let sender = CountingFakeSender::new(false);
        let processor = NotificationProcessor::new(&store, &sender, 3);
        let req = sample_request("evt-dedup-001");

        let first = processor.process_request(&req).await.expect("first run");
        assert_eq!(
            first,
            MessageDisposition::Complete {
                event_id: "evt-dedup-001".to_string(),
                duplicate_skipped: false,
                final_status: JobStatus::Delivered,
            }
        );
        assert_eq!(sender.calls.load(Ordering::SeqCst), 1);

        // Verify recipient raw email was erased on Delivered and redacted form kept
        let stored = store
            .get_job("evt-dedup-001")
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(stored.document.recipient_delivery, None);
        assert_eq!(stored.document.recipient_redacted, "s***@example.edu");

        // Second delivery of the exact same eventId must complete immediately without calling sender
        let second = processor
            .process_request(&req)
            .await
            .expect("duplicate run");
        assert_eq!(
            second,
            MessageDisposition::Complete {
                event_id: "evt-dedup-001".to_string(),
                duplicate_skipped: true,
                final_status: JobStatus::Delivered,
            }
        );
        assert_eq!(sender.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retries_transient_failure_with_optimistic_etag_reclaim() {
        let store = InMemoryCosmosStore::new();
        let sender = CountingFakeSender::new(true);
        let processor = NotificationProcessor::new(&store, &sender, 3);
        let req = sample_request("evt-retry-001");

        let first = processor
            .process_request(&req)
            .await
            .expect("first attempt");
        assert!(matches!(first, MessageDisposition::Wait { .. }));

        let after_first = store
            .get_job("evt-retry-001")
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(after_first.document.status, JobStatus::RetryableFailure);
        assert_eq!(after_first.document.attempt_count, 1);

        assert!(matches!(
            processor.process_request(&req).await.unwrap(),
            MessageDisposition::Wait { .. }
        ));
        assert_eq!(sender.calls.load(Ordering::SeqCst), 1);
        store
            .docs
            .lock()
            .unwrap()
            .get_mut(&req.event_id)
            .unwrap()
            .document
            .next_retry_at = Some(Utc::now() - chrono::Duration::seconds(1));
        let second = processor
            .process_request(&req)
            .await
            .expect("second attempt");
        assert_eq!(
            second,
            MessageDisposition::Complete {
                event_id: "evt-retry-001".to_string(),
                duplicate_skipped: false,
                final_status: JobStatus::Delivered,
            }
        );

        let after_second = store
            .get_job("evt-retry-001")
            .await
            .expect("get")
            .expect("exists");
        assert_eq!(after_second.document.status, JobStatus::Delivered);
        assert_eq!(after_second.document.attempt_count, 2);
        assert_eq!(after_second.document.attempts.len(), 2);
    }
    struct PausedSender {
        started: tokio::sync::Notify,
        release: tokio::sync::Notify,
        calls: AtomicUsize,
    }

    #[async_trait]
    impl EmailDeliveryPort for &PausedSender {
        fn configured_mode(&self) -> &str {
            "fake"
        }
        async fn send_notification(
            &self,
            req: &NotificationRequestV1,
        ) -> Result<EmailDeliveryResult, DeliveryError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.started.notify_one();
            self.release.notified().await;
            Ok(EmailDeliveryResult {
                provider_message_id: req.event_id.clone(),
                delivery_mode: "fake".into(),
            })
        }
    }

    fn paused_sender() -> PausedSender {
        PausedSender {
            started: tokio::sync::Notify::new(),
            release: tokio::sync::Notify::new(),
            calls: AtomicUsize::new(0),
        }
    }

    #[tokio::test]
    async fn two_workers_cannot_send_under_a_live_lease() {
        let store = InMemoryCosmosStore::new();
        let sender = paused_sender();
        let first = NotificationProcessor::new(&store, &sender, 3);
        let second = NotificationProcessor::new(&store, &sender, 3);
        let req = sample_request("compete");
        let (outcome, ()) = tokio::join!(first.process_request(&req), async {
            sender.started.notified().await;
            assert!(matches!(
                second.process_request(&req).await.unwrap(),
                MessageDisposition::Wait { .. }
            ));
            assert_eq!(sender.calls.load(Ordering::SeqCst), 1);
            sender.release.notify_one();
        });
        assert!(matches!(
            outcome.unwrap(),
            MessageDisposition::Complete { .. }
        ));
    }

    #[tokio::test]
    async fn expired_claim_recovers_and_rejects_stale_completion() {
        let store = InMemoryCosmosStore::new();
        let sender = CountingFakeSender::new(false);
        let req = sample_request("expired");
        let old = NotificationJobDocument::new_claimed(
            &req,
            "fake",
            Utc::now() - chrono::Duration::seconds(120),
        );
        let CreateJobOutcome::Created(stale) = store.create_job(&old).await.unwrap() else {
            panic!()
        };
        let processor = NotificationProcessor::new(&store, &sender, 3);
        assert!(matches!(
            processor.process_request(&req).await.unwrap(),
            MessageDisposition::Complete { .. }
        ));
        let mut stale_doc = stale.document;
        stale_doc.mark_delivered("old".into(), "fake", Utc::now());
        assert_eq!(
            store.replace_job(&stale_doc, &stale.etag).await.unwrap(),
            ReplaceJobOutcome::PreconditionFailed
        );
        assert_eq!(sender.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn lost_cosmos_renewal_cancels_send() {
        let store = InMemoryCosmosStore::new();
        let sender = paused_sender();
        let processor = NotificationProcessor::new(&store, &sender, 3);
        let req = sample_request("lost-renewal");
        let (outcome, ()) = tokio::join!(processor.process_request(&req), async {
            sender.started.notified().await;
            let mut current = store.get_job(&req.event_id).await.unwrap().unwrap();
            current.document.claim_token = Some("other-owner".into());
            store
                .replace_job(&current.document, &current.etag)
                .await
                .unwrap();
            tokio::time::advance(std::time::Duration::from_secs(11)).await;
        });
        assert!(matches!(outcome.unwrap(), MessageDisposition::Wait { .. }));
        assert_eq!(
            store
                .get_job(&req.event_id)
                .await
                .unwrap()
                .unwrap()
                .document
                .status,
            JobStatus::Claimed
        );
    }

    #[tokio::test(start_paused = true)]
    async fn timeout_is_durable_and_budget_is_bounded() {
        let store = InMemoryCosmosStore::new();
        let sender = paused_sender();
        let processor = NotificationProcessor::new(&store, &sender, 1);
        let req = sample_request("timeout");
        let result = processor.process_request(&req).await.unwrap();
        assert!(matches!(
            result,
            MessageDisposition::Complete {
                final_status: JobStatus::TerminalFailure,
                ..
            }
        ));
        assert!(matches!(
            processor.process_request(&req).await.unwrap(),
            MessageDisposition::Complete {
                duplicate_skipped: true,
                ..
            }
        ));
        assert_eq!(sender.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn crash_budget_does_not_allow_an_extra_send() {
        let store = InMemoryCosmosStore::new();
        let sender = CountingFakeSender::new(false);
        let req = sample_request("crash-budget");
        store
            .create_job(&NotificationJobDocument::new_claimed(
                &req,
                "fake",
                Utc::now() - chrono::Duration::seconds(120),
            ))
            .await
            .unwrap();
        let processor = NotificationProcessor::new(&store, &sender, 1);
        assert!(matches!(
            processor.process_request(&req).await.unwrap(),
            MessageDisposition::Complete {
                final_status: JobStatus::TerminalFailure,
                ..
            }
        ));
        assert_eq!(sender.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn malformed_and_unsupported_messages_do_not_send() {
        let store = InMemoryCosmosStore::new();
        let sender = CountingFakeSender::new(false);
        let processor = NotificationProcessor::new(&store, &sender, 3);
        let mut req = sample_request("unsupported");
        req.version = 999;
        for body in ["not json".into(), serde_json::to_string(&req).unwrap()] {
            assert!(matches!(
                processor.process_raw_message(&body).await.unwrap(),
                MessageDisposition::Abandon { .. }
            ));
        }
        assert_eq!(sender.calls.load(Ordering::SeqCst), 0);
        assert!(store.docs.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn legacy_terminal_documents_remain_deduplicated() {
        let store = InMemoryCosmosStore::new();
        let sender = CountingFakeSender::new(false);
        let req = sample_request("legacy");
        let mut doc = NotificationJobDocument::new_claimed(&req, "fake", Utc::now());
        doc.mark_delivered("old".into(), "fake", Utc::now());
        let mut json = serde_json::to_value(doc).unwrap();
        for field in ["claimToken", "leaseUntil", "nextRetryAt"] {
            json.as_object_mut().unwrap().remove(field);
        }
        store
            .create_job(&serde_json::from_value(json).unwrap())
            .await
            .unwrap();
        let processor = NotificationProcessor::new(&store, &sender, 3);
        assert!(matches!(
            processor.process_request(&req).await.unwrap(),
            MessageDisposition::Complete {
                duplicate_skipped: true,
                ..
            }
        ));
        assert_eq!(sender.calls.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn cancellation_after_send_starts_retains_recoverable_claim() {
        let store = InMemoryCosmosStore::new();
        let sender = paused_sender();
        let processor = NotificationProcessor::new(&store, &sender, 3);
        let req = sample_request("cancelled");
        {
            let processing = processor.process_request(&req);
            tokio::pin!(processing);
            tokio::select! {
                _ = sender.started.notified() => {}
                _ = &mut processing => panic!("send should pause"),
            }
        }
        let job = store.get_job(&req.event_id).await.unwrap().unwrap();
        assert_eq!(job.document.status, JobStatus::Claimed);
        assert!(job.document.lease_until.is_some());
        assert!(matches!(
            processor.process_request(&req).await.unwrap(),
            MessageDisposition::Wait { .. }
        ));
    }

    #[tokio::test]
    async fn all_event_types_and_subscribers_drain_after_worker_outage() {
        let store = InMemoryCosmosStore::new();
        let sender = CountingFakeSender::new(false);
        let mut retained = Vec::new();
        for (i, kind) in [
            rc_shared::EVENT_TYPE_COLLECTION_RECORDED,
            rc_shared::EVENT_TYPE_RETURN_RECORDED,
            EVENT_TYPE_AVAILABILITY_REQUESTED,
            EVENT_TYPE_AVAILABILITY_REQUESTED,
        ]
        .iter()
        .enumerate()
        {
            let mut req = sample_request(&format!("backlog-{i}"));
            req.event_type = (*kind).into();
            req.recipient = format!("subscriber-{i}@example.invalid");
            retained.push(serde_json::to_string(&req).unwrap());
        }
        let processor = NotificationProcessor::new(&store, &sender, 3);
        for body in &retained {
            assert!(matches!(
                processor.process_raw_message(body).await.unwrap(),
                MessageDisposition::Complete {
                    duplicate_skipped: false,
                    ..
                }
            ));
        }
        for body in &retained {
            assert!(matches!(
                processor.process_raw_message(body).await.unwrap(),
                MessageDisposition::Complete {
                    duplicate_skipped: true,
                    ..
                }
            ));
        }
        assert_eq!(sender.calls.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn send_success_without_record_can_send_again_after_expiry() {
        // Models the crash gap after external acceptance but before durable finish.
        // This test deliberately proves the residual duplicate risk, not exactly-once.
        let store = InMemoryCosmosStore::new();
        let sender = CountingFakeSender::new(false);
        let req = sample_request("uncertain-send");
        let old = NotificationJobDocument::new_claimed(
            &req,
            "fake",
            Utc::now() - chrono::Duration::seconds(120),
        );
        store.create_job(&old).await.unwrap();
        (&sender).send_notification(&req).await.unwrap();
        let processor = NotificationProcessor::new(&store, &sender, 3);
        processor.process_request(&req).await.unwrap();
        assert_eq!(sender.calls.load(Ordering::SeqCst), 2);
    }
}
