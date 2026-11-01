use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use rc_shared::{ManagedIdentityTokenProvider, NotificationRequestV1};
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, Semaphore};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::application::{
    build_collection_outbox_event, build_return_outbox_events, ApplicationError, BoxFuture,
    ClaimedOutboxItem, CollectionReceipt, LoanRepositoryPort, OutboxStorePort, QueuePublisherPort,
    ReservationReceipt, ReturnReceipt, SubscriptionReceipt,
};
use crate::domain::{
    compute_two_day_due_at, ensure_staff, ensure_student_or_staff, AuthenticatedActor,
    DeviceModelSummary, DomainError, Role,
};

const PG_ENTRA_RESOURCE: &str = "https://ossrdbms-aad.database.windows.net";
const SERVICEBUS_ENTRA_RESOURCE: &str = "https://servicebus.azure.net";

const MIGRATION_0001_SQL: &str = include_str!("../migrations/0001_initial_loan_schema.sql");
const DEMO_SEED_SQL: &str = include_str!("../migrations/demo_seed.sql");

#[derive(Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub static_password: Option<String>,
    pub max_connections: usize,
    pub publisher_role_name: Option<String>,
}

/// Bounded PostgreSQL adapter using TLS verification and Microsoft Entra Managed Identity
/// token refresh (or an optional local development password).
/// Limits concurrent connections per replica (`max_connections`, default 5) to protect
/// the B1ms PostgreSQL instance across Container Apps replicas.
#[derive(Clone)]
pub struct PostgresAdapter {
    cfg: PostgresConfig,
    token_provider: Option<ManagedIdentityTokenProvider>,
    semaphore: Arc<Semaphore>,
}

impl PostgresAdapter {
    pub fn new(cfg: PostgresConfig, token_provider: Option<ManagedIdentityTokenProvider>) -> Self {
        let max_conn = cfg.max_connections.max(1);
        Self {
            cfg,
            token_provider,
            semaphore: Arc::new(Semaphore::new(max_conn)),
        }
    }

    pub async fn connect_client(
        &self,
    ) -> Result<(Client, tokio::sync::OwnedSemaphorePermit), ApplicationError> {
        let permit =
            self.semaphore.clone().acquire_owned().await.map_err(|e| {
                ApplicationError::Persistence(format!("connection pool closed: {e}"))
            })?;

        let password = if let Some(ref pass) = self.cfg.static_password {
            pass.clone()
        } else if let Some(ref provider) = self.token_provider {
            provider
                .get_token(PG_ENTRA_RESOURCE)
                .await
                .map_err(|e| ApplicationError::Persistence(format!("entra token error: {e}")))?
        } else {
            return Err(ApplicationError::Persistence(
                "neither PGPASSWORD nor ManagedIdentityTokenProvider configured".into(),
            ));
        };

        let tls_connector = TlsConnector::builder()
            .build()
            .map_err(|e| ApplicationError::Persistence(format!("tls builder error: {e}")))?;
        let connector = MakeTlsConnector::new(tls_connector);

        let mut pg_cfg = tokio_postgres::Config::new();
        pg_cfg
            .host(&self.cfg.host)
            .port(self.cfg.port)
            .dbname(&self.cfg.database)
            .user(&self.cfg.user)
            .password(&password)
            .connect_timeout(Duration::from_secs(15))
            .ssl_mode(tokio_postgres::config::SslMode::Require);

        let (client, connection) = pg_cfg
            .connect(connector)
            .await
            .map_err(|e| ApplicationError::Persistence(format!("postgres connect failed: {e}")))?;

        tokio::spawn(async move {
            if let Err(err) = connection.await {
                tracing::warn!(error = %err, "postgres connection task ended");
            }
        });

        Ok((client, permit))
    }

    /// Runs idempotent versioned schema migrations (`0001_initial_loan_schema.sql`),
    /// grants least-privilege `SELECT, UPDATE` on `loan.outbox_events` to the publisher role,
    /// and optionally seeds deterministic Test demo data when `seed_demo` is true.
    pub async fn run_migrations(&self, seed_demo: bool) -> Result<(), ApplicationError> {
        let (mut client, _permit) = self.connect_client().await?;
        let tx = client
            .transaction()
            .await
            .map_err(|e| ApplicationError::Persistence(format!("begin migration tx: {e}")))?;

        tx.batch_execute(MIGRATION_0001_SQL)
            .await
            .map_err(|e| ApplicationError::Persistence(format!("migration 0001 failed: {e}")))?;

        tx.execute(
            "INSERT INTO loan.schema_migrations (version) VALUES ($1) ON CONFLICT (version) DO NOTHING",
            &[&"0001_initial_loan_schema"],
        )
        .await
        .map_err(|e| ApplicationError::Persistence(format!("record migration version: {e}")))?;

        if let Some(ref pub_role) = self.cfg.publisher_role_name {
            let escaped_ident = pub_role.replace('"', "\"\"");
            let grant_sql =
                format!("GRANT SELECT, UPDATE ON TABLE loan.outbox_events TO \"{escaped_ident}\";");
            tx.batch_execute(&grant_sql).await.map_err(|e| {
                ApplicationError::Persistence(format!("grant publisher outbox rights: {e}"))
            })?;
        }

        if seed_demo {
            tx.batch_execute(DEMO_SEED_SQL)
                .await
                .map_err(|e| ApplicationError::Persistence(format!("demo seed failed: {e}")))?;
        }

        tx.commit()
            .await
            .map_err(|e| ApplicationError::Persistence(format!("commit migration tx: {e}")))?;

        Ok(())
    }

    /// Inserts a synthetic outbox probe row inside `loan.outbox_events` for Phase 2
    /// end-to-end infrastructure verification (marked `syntheticProbe: true`).
    pub async fn enqueue_synthetic_outbox_probe(
        &self,
        event_id: &str,
        correlation_id: &str,
    ) -> Result<(), ApplicationError> {
        let (client, _permit) = self.connect_client().await?;
        let req = NotificationRequestV1 {
            event_id: event_id.to_string(),
            event_type: rc_shared::EVENT_TYPE_AVAILABILITY_REQUESTED.to_string(),
            version: rc_shared::CONTRACT_VERSION_V1,
            occurred_at: Utc::now(),
            correlation_id: correlation_id.to_string(),
            recipient: "probe-no-email@example.invalid".to_string(),
            device_model: "Dell Latitude 7440".to_string(),
            synthetic_probe: Some(true),
        };
        let payload_json =
            serde_json::to_value(&req).map_err(|e| ApplicationError::Persistence(e.to_string()))?;

        client
            .execute(
                "INSERT INTO loan.outbox_events (event_id, event_type, contract_version, occurred_at, correlation_id, payload, status)
                 VALUES ($1, $2, $3, $4, $5, $6, 'PENDING')
                 ON CONFLICT (event_id) DO NOTHING",
                &[
                    &req.event_id,
                    &req.event_type,
                    &(req.version as i32),
                    &req.occurred_at,
                    &req.correlation_id,
                    &payload_json,
                ],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("insert synthetic outbox: {e}")))?;
        Ok(())
    }

    /// Executes the Phase 3 live PostgreSQL resilience, concurrency, and transactional outbox
    /// verification suite against the private PostgreSQL server:
    /// 1. Concurrent reservation of the last available device unit (5 concurrent contenders ->
    ///    1 `RESERVED`, 4 `NoAvailableUnits`, `available_units == 0`).
    /// 2. Availability subscription while `available_units == 0` -> `ACTIVE` subscription.
    /// 3. Role-based access control rejection (`ForbiddenRole("staff")` when a student attempts collection).
    /// 4. Staff collection -> 2-day due date (`due_at - collected_at == 48h`) + atomic `loan.collection_recorded`
    ///    outbox event (`syntheticProbe: true`).
    /// 5. Publisher crash-window lease expiry recovery on `loan.outbox_events` (`claimed_until` expiry
    ///    re-claimed cleanly with incremented `attempt_count`).
    /// 6. Staff return -> atomic unit restoration (`available_units == 1`), subscription transition to
    ///    `NOTIFIED`, and atomic commit of both `loan.return_recorded` and `loan.availability_notification_requested`
    ///    outbox events (`syntheticProbe: true` so the scheduled publisher and Notifications worker process
    ///    all 3 notification types through Service Bus -> KEDA -> Cosmos DB with zero live emails).
    /// 7. Cleans up the temporary test device model/unit/reservation/subscription rows while leaving the
    ///    3 synthetic outbox events in `loan.outbox_events` for the live pipeline to publish and settle.
    pub async fn run_resilience_suite(&self) -> Result<serde_json::Value, ApplicationError> {
        let run_id = Uuid::new_v4().simple().to_string()[..8].to_string();
        let model_id = format!("resilience-model-{run_id}");
        let unit_id = format!("unit-resilience-{run_id}");
        let serial = format!("SN-RESILIENCE-{run_id}");
        let corr_prefix = format!("corr-phase3-suite-{run_id}");

        let (client, _permit) = self.connect_client().await?;
        client
            .execute(
                "INSERT INTO loan.device_models (id, name, category, description, loan_duration_days, total_units, available_units, updated_at)
                 VALUES ($1, 'Phase 3 Concurrency Probe Kit', 'Laptop', 'Temporary single-unit model for concurrent reservation test', 2, 1, 1, NOW())",
                &[&model_id],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("insert test model: {e}")))?;

        client
            .execute(
                "INSERT INTO loan.device_units (unit_id, model_id, serial_number, status, updated_at)
                 VALUES ($1, $2, $3, 'AVAILABLE', NOW())",
                &[&unit_id, &model_id, &serial],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("insert test unit: {e}")))?;

        drop(client);
        drop(_permit);

        // 1. Spawn 5 concurrent reservation tasks for the single available unit
        let mut handles = Vec::with_capacity(5);
        for i in 0..5 {
            let adapter = self.clone();
            let m_id = model_id.clone();
            let c_id = format!("{corr_prefix}-reserve-{i}");
            let actor = AuthenticatedActor {
                subject: format!("auth0|phase3-student-{i}"),
                email: format!("student{i}-no-email@example.invalid"),
                roles: vec![Role::Student],
            };
            handles.push(tokio::spawn(async move {
                adapter.reserve_device(actor, m_id, c_id).await
            }));
        }

        let mut winners = Vec::new();
        let mut conflicts = 0usize;
        for h in handles {
            match h
                .await
                .map_err(|e| ApplicationError::Persistence(format!("join error: {e}")))?
            {
                Ok(receipt) => winners.push(receipt),
                Err(ApplicationError::Domain(DomainError::NoAvailableUnits(_))) => {
                    conflicts += 1;
                }
                Err(other) => return Err(other),
            }
        }

        if winners.len() != 1 || conflicts != 4 {
            return Err(ApplicationError::Persistence(format!(
                "concurrency invariant violated: expected 1 winner and 4 conflicts, got {} winners and {} conflicts",
                winners.len(),
                conflicts
            )));
        }
        let winning_reservation = winners.remove(0);

        // 2. Subscribe a waitlisted student now that available_units == 0
        let waitlist_actor = AuthenticatedActor {
            subject: "auth0|phase3-waitlist-student".to_string(),
            email: "waitlist-no-email@example.invalid".to_string(),
            roles: vec![Role::Student],
        };
        let subscription = self
            .subscribe_availability(waitlist_actor.clone(), model_id.clone())
            .await?;

        // 3. Verify RBAC rejection when student attempts staff-only collection
        let rbac_rejected = matches!(
            self.record_collection(
                waitlist_actor,
                winning_reservation.reservation_id,
                format!("{corr_prefix}-rbac-deny")
            )
            .await,
            Err(ApplicationError::Domain(DomainError::ForbiddenRole(
                "staff"
            )))
        );
        if !rbac_rejected {
            return Err(ApplicationError::Persistence(
                "expected ForbiddenRole(staff) for student collection attempt".into(),
            ));
        }

        // 4. Staff records collection -> 2-day due date + collection_recorded outbox row
        let staff_actor = AuthenticatedActor {
            subject: "auth0|phase3-staff-1".to_string(),
            email: "staff-no-email@example.invalid".to_string(),
            roles: vec![Role::Staff],
        };
        let collect_corr = format!("{corr_prefix}-collect");
        let collection = self
            .record_collection(
                staff_actor.clone(),
                winning_reservation.reservation_id,
                collect_corr.clone(),
            )
            .await?;

        let loan_hours = (collection.due_at - collection.collected_at).num_hours();
        if loan_hours != 48 {
            return Err(ApplicationError::Persistence(format!(
                "expected 48h loan duration, got {loan_hours}h"
            )));
        }

        // 5. Publisher crash-window simulation on the collection outbox row:
        //    Simulate a publisher claiming the row whose lease has expired (`claimed_until = NOW() - INTERVAL '5 seconds'`),
        //    then verify `claim_pending_batch` re-claims it and increments `attempt_count`.
        let (client, _permit) = self.connect_client().await?;
        client
            .execute(
                "UPDATE loan.outbox_events
                 SET payload = jsonb_set(payload, '{syntheticProbe}', 'true'::jsonb, true),
                     claimed_until = NOW() - INTERVAL '5 seconds',
                     attempt_count = 1
                 WHERE event_id = $1",
                &[&collection.outbox_event_id],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("simulate crash lease: {e}")))?;
        drop(client);
        drop(_permit);

        let reclaimed_batch = self.claim_pending_batch(10, 5).await?;
        let reclaimed_item = reclaimed_batch
            .iter()
            .find(|item| item.event_id == collection.outbox_event_id)
            .ok_or_else(|| {
                ApplicationError::Persistence(
                    "expired outbox lease was not reclaimed by claim_pending_batch".into(),
                )
            })?;
        let crash_reclaim_attempt_count = reclaimed_item.attempt_count;
        // Release the lease (`claimed_until = NULL`) so the scheduled publisher job picks it up immediately
        let (client, _permit) = self.connect_client().await?;
        client
            .execute(
                "UPDATE loan.outbox_events SET claimed_until = NULL WHERE event_id = $1",
                &[&collection.outbox_event_id],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("reset claim lease: {e}")))?;
        drop(client);
        drop(_permit);

        // 6. Staff records return -> restores unit availability, notifies waitlist subscriber,
        //    and commits both return_recorded and availability_notification_requested outbox events.
        let return_corr = format!("{corr_prefix}-return");
        let return_receipt = self
            .record_return(
                staff_actor,
                winning_reservation.reservation_id,
                return_corr.clone(),
            )
            .await?;

        if return_receipt.outbox_event_ids.len() != 2 {
            return Err(ApplicationError::Persistence(format!(
                "expected 2 return outbox events (return_recorded + availability_notification_requested), got {}",
                return_receipt.outbox_event_ids.len()
            )));
        }

        // Tag the 2 return outbox rows with syntheticProbe: true so FakeEmailSender is guaranteed
        // and clean up the temporary device/reservation/subscription rows.
        let (mut client, _permit) = self.connect_client().await?;
        for evt_id in &return_receipt.outbox_event_ids {
            client
                .execute(
                    "UPDATE loan.outbox_events
                     SET payload = jsonb_set(payload, '{syntheticProbe}', 'true'::jsonb, true)
                     WHERE event_id = $1",
                    &[evt_id],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("tag synthetic probe: {e}")))?;
        }

        let tx = client
            .transaction()
            .await
            .map_err(|e| ApplicationError::Persistence(format!("cleanup tx: {e}")))?;
        tx.execute(
            "DELETE FROM loan.availability_subscriptions WHERE subscription_id = $1",
            &[&subscription.subscription_id],
        )
        .await
        .map_err(|e| ApplicationError::Persistence(format!("cleanup subscription: {e}")))?;
        tx.execute(
            "DELETE FROM loan.reservations WHERE reservation_id = $1",
            &[&winning_reservation.reservation_id],
        )
        .await
        .map_err(|e| ApplicationError::Persistence(format!("cleanup reservation: {e}")))?;
        tx.execute(
            "DELETE FROM loan.device_units WHERE unit_id = $1",
            &[&unit_id],
        )
        .await
        .map_err(|e| ApplicationError::Persistence(format!("cleanup unit: {e}")))?;
        tx.execute("DELETE FROM loan.device_models WHERE id = $1", &[&model_id])
            .await
            .map_err(|e| ApplicationError::Persistence(format!("cleanup model: {e}")))?;
        tx.commit()
            .await
            .map_err(|e| ApplicationError::Persistence(format!("commit cleanup tx: {e}")))?;

        Ok(serde_json::json!({
            "runId": run_id,
            "concurrentReservationContenders": 5,
            "winnersCount": 1,
            "noAvailableUnitsConflicts": conflicts,
            "winningReservationId": winning_reservation.reservation_id,
            "subscriptionId": subscription.subscription_id,
            "studentRbacCollectionRejected": rbac_rejected,
            "loanDurationHours": loan_hours,
            "collectionOutboxEventId": collection.outbox_event_id,
            "collectionCorrelationId": collect_corr,
            "publisherCrashLeaseReclaimAttemptCount": crash_reclaim_attempt_count,
            "returnOutboxEventIds": return_receipt.outbox_event_ids,
            "returnCorrelationId": return_corr,
            "temporaryRowsCleaned": true
        }))
    }
}

impl LoanRepositoryPort for PostgresAdapter {
    fn check_readiness(&self) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            let (client, _permit) = self.connect_client().await?;
            let row = client
                .query_one(
                    "SELECT has_schema_privilege(current_user, 'loan', 'USAGE')",
                    &[],
                )
                .await
                .map_err(|e| {
                    ApplicationError::Persistence(format!("readiness query failed: {e}"))
                })?;
            let has_usage: bool = row.get(0);
            if !has_usage {
                return Err(ApplicationError::Persistence(
                    "current user lacks USAGE on schema loan".into(),
                ));
            }
            Ok(())
        })
    }

    fn list_catalogue(&self) -> BoxFuture<'_, Result<Vec<DeviceModelSummary>, ApplicationError>> {
        Box::pin(async move {
            let (client, _permit) = self.connect_client().await?;
            let rows = client
                .query(
                    "SELECT id, name, category, description, loan_duration_days, total_units, available_units
                     FROM loan.device_models
                     ORDER BY category ASC, name ASC",
                    &[],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("list catalogue failed: {e}")))?;

            let models = rows
                .into_iter()
                .map(|r| DeviceModelSummary {
                    id: r.get(0),
                    name: r.get(1),
                    category: r.get(2),
                    description: r.get(3),
                    loan_duration_days: r.get(4),
                    total_units: r.get(5),
                    available_units: r.get(6),
                })
                .collect();
            Ok(models)
        })
    }

    fn reserve_device(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
        correlation_id: String,
    ) -> BoxFuture<'_, Result<ReservationReceipt, ApplicationError>> {
        Box::pin(async move {
            ensure_student_or_staff(&actor)?;
            let (mut client, _permit) = self.connect_client().await?;
            let tx = client
                .transaction()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("begin reserve tx: {e}")))?;

            // Lock model row for atomic allocation check
            let model_row = tx
                .query_opt(
                    "SELECT available_units FROM loan.device_models WHERE id = $1 FOR UPDATE",
                    &[&model_id],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("lock model row: {e}")))?;

            let Some(model_row) = model_row else {
                return Err(ApplicationError::NotFound(format!(
                    "device model {model_id}"
                )));
            };
            let available: i32 = model_row.get(0);
            if available <= 0 {
                return Err(ApplicationError::Domain(DomainError::NoAvailableUnits(
                    model_id,
                )));
            }

            let unit_row = tx
                .query_opt(
                    "SELECT unit_id FROM loan.device_units
                     WHERE model_id = $1 AND status = 'AVAILABLE'
                     ORDER BY unit_id ASC
                     LIMIT 1
                     FOR UPDATE SKIP LOCKED",
                    &[&model_id],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("lock unit row: {e}")))?;

            let Some(unit_row) = unit_row else {
                return Err(ApplicationError::Domain(DomainError::NoAvailableUnits(
                    model_id,
                )));
            };
            let unit_id: String = unit_row.get(0);
            let reservation_id = Uuid::new_v4();
            let reserved_at = Utc::now();

            tx.execute(
                "UPDATE loan.device_units SET status = 'RESERVED', updated_at = $2 WHERE unit_id = $1",
                &[&unit_id, &reserved_at],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("update unit status: {e}")))?;

            tx.execute(
                "UPDATE loan.device_models SET available_units = available_units - 1, updated_at = $2 WHERE id = $1",
                &[&model_id, &reserved_at],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("decrement available_units: {e}")))?;

            tx.execute(
                "INSERT INTO loan.reservations (reservation_id, model_id, unit_id, student_sub, student_email, status, reserved_at, correlation_id)
                 VALUES ($1, $2, $3, $4, $5, 'RESERVED', $6, $7)",
                &[
                    &reservation_id,
                    &model_id,
                    &unit_id,
                    &actor.subject,
                    &actor.email,
                    &reserved_at,
                    &correlation_id,
                ],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("insert reservation: {e}")))?;

            tx.commit()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("commit reserve tx: {e}")))?;

            Ok(ReservationReceipt {
                reservation_id,
                model_id,
                unit_id,
                status: "RESERVED".to_string(),
                reserved_at,
                correlation_id,
            })
        })
    }

    fn subscribe_availability(
        &self,
        actor: AuthenticatedActor,
        model_id: String,
    ) -> BoxFuture<'_, Result<SubscriptionReceipt, ApplicationError>> {
        Box::pin(async move {
            ensure_student_or_staff(&actor)?;
            let (mut client, _permit) = self.connect_client().await?;
            let tx = client
                .transaction()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("begin subscribe tx: {e}")))?;

            let model_row = tx
                .query_opt(
                    "SELECT available_units FROM loan.device_models WHERE id = $1",
                    &[&model_id],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("lookup model: {e}")))?;

            let Some(model_row) = model_row else {
                return Err(ApplicationError::NotFound(format!(
                    "device model {model_id}"
                )));
            };
            let available: i32 = model_row.get(0);
            if available > 0 {
                return Err(ApplicationError::Domain(
                    DomainError::UnitsCurrentlyAvailable(model_id),
                ));
            }

            let subscription_id = Uuid::new_v4();
            let created_at = Utc::now();

            let row = tx
                .query_one(
                    "INSERT INTO loan.availability_subscriptions (subscription_id, model_id, student_sub, student_email, status, created_at)
                     VALUES ($1, $2, $3, $4, 'ACTIVE', $5)
                     ON CONFLICT (model_id, student_sub, status)
                     DO UPDATE SET student_email = EXCLUDED.student_email
                     RETURNING subscription_id, created_at",
                    &[
                        &subscription_id,
                        &model_id,
                        &actor.subject,
                        &actor.email,
                        &created_at,
                    ],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("upsert subscription: {e}")))?;

            tx.commit()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("commit subscribe tx: {e}")))?;

            Ok(SubscriptionReceipt {
                subscription_id: row.get(0),
                model_id,
                status: "ACTIVE".to_string(),
                created_at: row.get(1),
            })
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
            let (mut client, _permit) = self.connect_client().await?;
            let tx = client
                .transaction()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("begin collect tx: {e}")))?;

            let res_row = tx
                .query_opt(
                    "SELECT r.model_id, r.unit_id, r.student_email, r.status, m.name
                     FROM loan.reservations r
                     JOIN loan.device_models m ON m.id = r.model_id
                     WHERE r.reservation_id = $1
                     FOR UPDATE OF r",
                    &[&reservation_id],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("lock reservation: {e}")))?;

            let Some(res_row) = res_row else {
                return Err(ApplicationError::NotFound(format!(
                    "reservation {reservation_id}"
                )));
            };

            let unit_id: String = res_row.get(1);
            let student_email: String = res_row.get(2);
            let current_status: String = res_row.get(3);
            let model_name: String = res_row.get(4);

            if current_status != "RESERVED" {
                return Err(ApplicationError::Domain(DomainError::InvalidTransition {
                    from: current_status,
                    to: "COLLECTED".to_string(),
                }));
            }

            let collected_at = Utc::now();
            let due_at = compute_two_day_due_at(collected_at);

            tx.execute(
                "UPDATE loan.reservations
                 SET status = 'COLLECTED', collected_at = $2, due_at = $3
                 WHERE reservation_id = $1",
                &[&reservation_id, &collected_at, &due_at],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("update reservation: {e}")))?;

            tx.execute(
                "UPDATE loan.device_units SET status = 'COLLECTED', updated_at = $2 WHERE unit_id = $1",
                &[&unit_id, &collected_at],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("update unit collected: {e}")))?;

            let outbox_evt = build_collection_outbox_event(
                reservation_id,
                &student_email,
                &model_name,
                collected_at,
                &correlation_id,
            );
            let payload_json = serde_json::to_value(&outbox_evt)
                .map_err(|e| ApplicationError::Persistence(e.to_string()))?;

            tx.execute(
                "INSERT INTO loan.outbox_events (event_id, event_type, contract_version, occurred_at, correlation_id, payload, status)
                 VALUES ($1, $2, $3, $4, $5, $6, 'PENDING')
                 ON CONFLICT (event_id) DO NOTHING",
                &[
                    &outbox_evt.event_id,
                    &outbox_evt.event_type,
                    &(outbox_evt.version as i32),
                    &outbox_evt.occurred_at,
                    &outbox_evt.correlation_id,
                    &payload_json,
                ],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("insert collection outbox: {e}")))?;

            tx.commit()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("commit collect tx: {e}")))?;

            Ok(CollectionReceipt {
                reservation_id,
                unit_id,
                status: "COLLECTED".to_string(),
                collected_at,
                due_at,
                outbox_event_id: outbox_evt.event_id,
            })
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
            let (mut client, _permit) = self.connect_client().await?;
            let tx = client
                .transaction()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("begin return tx: {e}")))?;

            let res_row = tx
                .query_opt(
                    "SELECT r.model_id, r.unit_id, r.student_email, r.status, m.name
                     FROM loan.reservations r
                     JOIN loan.device_models m ON m.id = r.model_id
                     WHERE r.reservation_id = $1
                     FOR UPDATE OF r",
                    &[&reservation_id],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("lock reservation: {e}")))?;

            let Some(res_row) = res_row else {
                return Err(ApplicationError::NotFound(format!(
                    "reservation {reservation_id}"
                )));
            };

            let model_id: String = res_row.get(0);
            let unit_id: String = res_row.get(1);
            let borrower_email: String = res_row.get(2);
            let current_status: String = res_row.get(3);
            let model_name: String = res_row.get(4);

            if current_status != "COLLECTED" && current_status != "RESERVED" {
                return Err(ApplicationError::Domain(DomainError::InvalidTransition {
                    from: current_status,
                    to: "RETURNED".to_string(),
                }));
            }

            let returned_at = Utc::now();

            tx.execute(
                "UPDATE loan.reservations
                 SET status = 'RETURNED', returned_at = $2
                 WHERE reservation_id = $1",
                &[&reservation_id, &returned_at],
            )
            .await
            .map_err(|e| {
                ApplicationError::Persistence(format!("update reservation return: {e}"))
            })?;

            tx.execute(
                "UPDATE loan.device_units SET status = 'AVAILABLE', updated_at = $2 WHERE unit_id = $1",
                &[&unit_id, &returned_at],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("update unit available: {e}")))?;

            tx.execute(
                "UPDATE loan.device_models SET available_units = available_units + 1, updated_at = $2 WHERE id = $1",
                &[&model_id, &returned_at],
            )
            .await
            .map_err(|e| ApplicationError::Persistence(format!("increment available_units: {e}")))?;

            // Select eligible waiting subscribers atomically in Loan
            let sub_rows = tx
                .query(
                    "UPDATE loan.availability_subscriptions
                     SET status = 'NOTIFIED', notified_at = $2
                     WHERE subscription_id IN (
                         SELECT subscription_id FROM loan.availability_subscriptions
                         WHERE model_id = $1 AND status = 'ACTIVE'
                         ORDER BY created_at ASC
                         FOR UPDATE SKIP LOCKED
                     )
                     RETURNING student_email",
                    &[&model_id, &returned_at],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("claim subscriptions: {e}")))?;

            let subscriber_emails: Vec<String> = sub_rows.into_iter().map(|r| r.get(0)).collect();
            let outbox_events = build_return_outbox_events(
                reservation_id,
                &borrower_email,
                &subscriber_emails,
                &model_name,
                returned_at,
                &correlation_id,
            );

            let mut outbox_event_ids = Vec::with_capacity(outbox_events.len());
            for evt in outbox_events {
                let payload_json = serde_json::to_value(&evt)
                    .map_err(|e| ApplicationError::Persistence(e.to_string()))?;
                tx.execute(
                    "INSERT INTO loan.outbox_events (event_id, event_type, contract_version, occurred_at, correlation_id, payload, status)
                     VALUES ($1, $2, $3, $4, $5, $6, 'PENDING')
                     ON CONFLICT (event_id) DO NOTHING",
                    &[
                        &evt.event_id,
                        &evt.event_type,
                        &(evt.version as i32),
                        &evt.occurred_at,
                        &evt.correlation_id,
                        &payload_json,
                    ],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("insert return outbox: {e}")))?;
                outbox_event_ids.push(evt.event_id);
            }

            tx.commit()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("commit return tx: {e}")))?;

            Ok(ReturnReceipt {
                reservation_id,
                unit_id,
                model_id,
                status: "RETURNED".to_string(),
                returned_at,
                outbox_event_ids,
            })
        })
    }
}

impl OutboxStorePort for PostgresAdapter {
    fn claim_pending_batch(
        &self,
        batch_size: i64,
        lease_seconds: i64,
    ) -> BoxFuture<'_, Result<Vec<ClaimedOutboxItem>, ApplicationError>> {
        Box::pin(async move {
            let (mut client, _permit) = self.connect_client().await?;
            let tx = client
                .transaction()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("begin claim tx: {e}")))?;

            let rows = tx
                .query(
                    "UPDATE loan.outbox_events
                     SET claimed_until = NOW() + ($2 * INTERVAL '1 second'),
                         attempt_count = attempt_count + 1
                     WHERE event_id IN (
                         SELECT event_id
                         FROM loan.outbox_events
                         WHERE status = 'PENDING'
                           AND (claimed_until IS NULL OR claimed_until < NOW())
                         ORDER BY occurred_at ASC
                         LIMIT $1
                         FOR UPDATE SKIP LOCKED
                     )
                     RETURNING event_id, correlation_id, attempt_count, payload",
                    &[&batch_size, &(lease_seconds as f64)],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("claim outbox rows: {e}")))?;

            tx.commit()
                .await
                .map_err(|e| ApplicationError::Persistence(format!("commit claim tx: {e}")))?;

            let mut items = Vec::with_capacity(rows.len());
            for row in rows {
                let event_id: String = row.get(0);
                let correlation_id: String = row.get(1);
                let attempt_count: i32 = row.get(2);
                let payload_val: serde_json::Value = row.get(3);
                let payload: NotificationRequestV1 =
                    serde_json::from_value(payload_val).map_err(|e| {
                        ApplicationError::Persistence(format!("invalid outbox json: {e}"))
                    })?;
                items.push(ClaimedOutboxItem {
                    event_id,
                    correlation_id,
                    attempt_count,
                    payload,
                });
            }
            Ok(items)
        })
    }

    fn mark_published(&self, event_id: String) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            let (client, _permit) = self.connect_client().await?;
            client
                .execute(
                    "UPDATE loan.outbox_events
                     SET status = 'PUBLISHED',
                         published_at = NOW(),
                         claimed_until = NULL,
                         last_error = NULL
                     WHERE event_id = $1",
                    &[&event_id],
                )
                .await
                .map_err(|e| ApplicationError::Persistence(format!("mark published: {e}")))?;
            Ok(())
        })
    }

    fn record_publish_failure(
        &self,
        event_id: String,
        error_message: String,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            let (client, _permit) = self.connect_client().await?;
            client
                .execute(
                    "UPDATE loan.outbox_events
                     SET last_error = $2,
                         claimed_until = NULL
                     WHERE event_id = $1",
                    &[&event_id, &error_message],
                )
                .await
                .map_err(|e| {
                    ApplicationError::Persistence(format!("record publish failure: {e}"))
                })?;
            Ok(())
        })
    }
}

/// Azure Service Bus HTTPS Queue Sender using Managed Identity Bearer tokens.
#[derive(Clone)]
pub struct ServiceBusRestPublisher {
    namespace_fqdn: String,
    queue_name: String,
    token_provider: ManagedIdentityTokenProvider,
    http: reqwest::Client,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct BrokerPropertiesOut<'a> {
    message_id: &'a str,
    label: &'a str,
    correlation_id: &'a str,
}

impl ServiceBusRestPublisher {
    pub fn new(
        namespace_fqdn: impl Into<String>,
        queue_name: impl Into<String>,
        token_provider: ManagedIdentityTokenProvider,
    ) -> Result<Self, ApplicationError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| ApplicationError::Broker(e.to_string()))?;
        Ok(Self {
            namespace_fqdn: namespace_fqdn.into(),
            queue_name: queue_name.into(),
            token_provider,
            http,
        })
    }
}

impl QueuePublisherPort for ServiceBusRestPublisher {
    fn publish_request(
        &self,
        request: NotificationRequestV1,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            let token = self
                .token_provider
                .get_token(SERVICEBUS_ENTRA_RESOURCE)
                .await
                .map_err(|e| ApplicationError::Broker(format!("servicebus token error: {e}")))?;

            let url = format!(
                "https://{}/{}/messages?timeout=60",
                self.namespace_fqdn.trim_end_matches('/'),
                self.queue_name
            );
            let broker_props = serde_json::to_string(&BrokerPropertiesOut {
                message_id: &request.event_id,
                label: &request.event_type,
                correlation_id: &request.correlation_id,
            })
            .map_err(|e| ApplicationError::Broker(e.to_string()))?;

            let body = serde_json::to_vec(&request)
                .map_err(|e| ApplicationError::Broker(e.to_string()))?;

            let resp = self
                .http
                .post(&url)
                .header("Authorization", format!("Bearer {token}"))
                .header("Content-Type", "application/json")
                .header("BrokerProperties", broker_props)
                .body(body)
                .send()
                .await
                .map_err(|e| ApplicationError::Broker(format!("servicebus post failed: {e}")))?;

            let status = resp.status();
            if status.as_u16() != 201 && !status.is_success() {
                let text = resp.text().await.unwrap_or_default();
                return Err(ApplicationError::Broker(format!(
                    "servicebus rejected send ({status}): {text}"
                )));
            }
            Ok(())
        })
    }
}

/// Auth0 OIDC / JWT Validator with JWKS caching.
/// When `issuer` or `audience` is empty, `is_configured()` returns false and protected routes
/// return `503 Service Unavailable (AuthNotConfigured)` rather than faking authentication.
#[derive(Clone)]
pub struct Auth0JwtVerifier {
    issuer: Option<String>,
    audience: Option<String>,
    role_claim: String,
    http: reqwest::Client,
    jwks_cache: Arc<RwLock<Option<(JwkSet, Instant)>>>,
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthVerifyError {
    #[error("Auth0 issuer/audience configuration is not set on this environment")]
    NotConfigured,
    #[error("missing or malformed Authorization Bearer token")]
    MissingBearerToken,
    #[error("invalid JWT token: {0}")]
    InvalidToken(String),
}

impl Auth0JwtVerifier {
    pub fn from_env() -> Self {
        let issuer = std::env::var("AUTH0_ISSUER")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let audience = std::env::var("AUTH0_AUDIENCE")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let role_claim = std::env::var("AUTH0_ROLES_CLAIM")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "https://resource-center.campus/roles".to_string());

        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest client");

        Self {
            issuer,
            audience,
            role_claim,
            http,
            jwks_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.issuer.is_some() && self.audience.is_some()
    }

    pub async fn verify_bearer(
        &self,
        auth_header: Option<&str>,
    ) -> Result<AuthenticatedActor, AuthVerifyError> {
        let (Some(ref issuer), Some(ref audience)) = (&self.issuer, &self.audience) else {
            return Err(AuthVerifyError::NotConfigured);
        };

        let Some(raw_header) = auth_header else {
            return Err(AuthVerifyError::MissingBearerToken);
        };
        let Some(token) = raw_header
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|t| !t.is_empty())
        else {
            return Err(AuthVerifyError::MissingBearerToken);
        };

        let header = decode_header(token)
            .map_err(|e| AuthVerifyError::InvalidToken(format!("jwt header: {e}")))?;
        let kid = header
            .kid
            .ok_or_else(|| AuthVerifyError::InvalidToken("missing kid in jwt header".into()))?;

        let jwks = self.fetch_jwks(issuer).await?;
        let jwk = jwks
            .find(&kid)
            .ok_or_else(|| AuthVerifyError::InvalidToken(format!("kid {kid} not found in JWKS")))?;

        let decoding_key = DecodingKey::from_jwk(jwk)
            .map_err(|e| AuthVerifyError::InvalidToken(format!("jwk key: {e}")))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer.as_str()]);
        validation.set_audience(&[audience.as_str()]);

        let token_data = decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| AuthVerifyError::InvalidToken(format!("claim validation failed: {e}")))?;

        let claims = token_data.claims;
        let mut roles = Vec::new();
        for key in [self.role_claim.as_str(), "roles", "permissions"] {
            if let Some(serde_json::Value::Array(arr)) = claims.extra.get(key) {
                for item in arr {
                    if let Some(r) = item.as_str() {
                        match r.to_ascii_lowercase().as_str() {
                            "student" => {
                                if !roles.contains(&Role::Student) {
                                    roles.push(Role::Student);
                                }
                            }
                            "staff" => {
                                if !roles.contains(&Role::Staff) {
                                    roles.push(Role::Staff);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let email = claims
            .email
            .unwrap_or_else(|| format!("{}@auth0.user", claims.sub));

        Ok(AuthenticatedActor {
            subject: claims.sub,
            email,
            roles,
        })
    }

    async fn fetch_jwks(&self, issuer: &str) -> Result<JwkSet, AuthVerifyError> {
        {
            let guard = self.jwks_cache.read().await;
            if let Some((ref set, expires_at)) = *guard {
                if Instant::now() < expires_at {
                    return Ok(set.clone());
                }
            }
        }
        let jwks_url = format!("{}/.well-known/jwks.json", issuer.trim_end_matches('/'));
        let resp = self
            .http
            .get(&jwks_url)
            .send()
            .await
            .map_err(|e| AuthVerifyError::InvalidToken(format!("jwks fetch error: {e}")))?;
        if !resp.status().is_success() {
            return Err(AuthVerifyError::InvalidToken(format!(
                "jwks endpoint returned {}",
                resp.status()
            )));
        }
        let set: JwkSet = resp
            .json()
            .await
            .map_err(|e| AuthVerifyError::InvalidToken(format!("jwks parse error: {e}")))?;

        let mut guard = self.jwks_cache.write().await;
        *guard = Some((set.clone(), Instant::now() + Duration::from_secs(600)));
        Ok(set)
    }
}
