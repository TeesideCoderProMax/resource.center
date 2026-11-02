use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use rc_shared::{ManagedIdentityTokenProvider, NotificationRequestV1};
use serde::Serialize;
use tokio::sync::Semaphore;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::application::{
    ApplicationError, BoxFuture, ClaimedOutboxItem, LoanRepositoryPort, LoanStorePort,
    LoanTransactionPort, OutboxStorePort, QueuePublisherPort, SubscriptionReceipt,
    TransactionAction,
};
use crate::domain::{
    AuthenticatedActor, DeviceModelSummary, DomainError, LockedModel, Reservation, Role,
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

        tx.batch_execute(
            "SELECT pg_advisory_xact_lock(73102401); CREATE SCHEMA IF NOT EXISTS loan;",
        )
        .await
        .map_err(pg_error)?;
        tx.batch_execute(MIGRATION_0001_SQL)
            .await
            .map_err(|e| ApplicationError::Persistence(format!("migration 0001 failed: {e}")))?;

        tx.execute(
            "INSERT INTO loan.schema_migrations (version) VALUES ($1) ON CONFLICT (version) DO NOTHING",
            &[&"0001_initial_loan_schema"],
        )
        .await
        .map_err(|e| ApplicationError::Persistence(format!("record migration version: {e}")))?;

        if tx.query_opt("SELECT version FROM loan.schema_migrations WHERE version = '0002_reservation_due_dates'", &[]).await.map_err(pg_error)?.is_none() {
            tx.batch_execute(include_str!("../migrations/0002_reservation_due_dates.sql")).await.map_err(pg_error)?;
            tx.execute("INSERT INTO loan.schema_migrations(version) VALUES ('0002_reservation_due_dates')", &[]).await.map_err(pg_error)?;
        }

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
    /// 4. Staff collection -> 2-day due date (`due_at - reserved_at == 48h`) + atomic `loan.collection_recorded`
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

        let loan_hours = (collection.due_at - winning_reservation.reserved_at).num_hours();
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

impl LoanStorePort for PostgresAdapter {
    fn readiness(&self) -> BoxFuture<'_, Result<(), ApplicationError>> {
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

    fn catalogue(&self) -> BoxFuture<'_, Result<Vec<DeviceModelSummary>, ApplicationError>> {
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

    fn reservations(
        &self,
        subject: Option<String>,
    ) -> BoxFuture<'_, Result<Vec<Reservation>, ApplicationError>> {
        Box::pin(async move {
            let (client, _permit) = self.connect_client().await?;
            let rows = client.query("SELECT * FROM loan.reservations WHERE ($1::text IS NULL OR student_sub = $1) ORDER BY reserved_at DESC, reservation_id LIMIT 200", &[&subject]).await.map_err(pg_error)?;
            Ok(rows.iter().map(reservation_from_row).collect())
        })
    }
    fn transact<T: Send + 'static>(
        &self,
        action: TransactionAction<T>,
    ) -> BoxFuture<'_, Result<T, ApplicationError>> {
        Box::pin(async move {
            let (mut client, _permit) = self.connect_client().await?;
            let tx = client.transaction().await.map_err(pg_error)?;
            let mut scope = PostgresLoanTransaction { tx };
            let result = action(&mut scope).await?;
            scope.tx.commit().await.map_err(pg_error)?;
            Ok(result)
        })
    }
}

fn pg_error(error: tokio_postgres::Error) -> ApplicationError {
    ApplicationError::Persistence(error.to_string())
}

fn reservation_from_row(row: &tokio_postgres::Row) -> Reservation {
    Reservation {
        reservation_id: row.get("reservation_id"),
        model_id: row.get("model_id"),
        unit_id: row.get("unit_id"),
        student_sub: row.get("student_sub"),
        student_email: row.get("student_email"),
        status: row.get("status"),
        reserved_at: row.get("reserved_at"),
        due_at: row.get("due_at"),
        collected_at: row.get("collected_at"),
        returned_at: row.get("returned_at"),
        correlation_id: row.get("correlation_id"),
    }
}

struct PostgresLoanTransaction<'a> {
    tx: tokio_postgres::Transaction<'a>,
}

impl LoanTransactionPort for PostgresLoanTransaction<'_> {
    fn lock_model(&mut self, id: String) -> BoxFuture<'_, Result<LockedModel, ApplicationError>> {
        Box::pin(async move {
            let row = self
                .tx
                .query_opt(
                    "SELECT name, available_units FROM loan.device_models WHERE id = $1 FOR UPDATE",
                    &[&id],
                )
                .await
                .map_err(pg_error)?
                .ok_or_else(|| ApplicationError::NotFound(format!("device model {id}")))?;
            Ok(LockedModel {
                id,
                name: row.get(0),
                available_units: row.get(1),
            })
        })
    }
    fn available_unit(
        &mut self,
        model: String,
    ) -> BoxFuture<'_, Result<Option<String>, ApplicationError>> {
        Box::pin(async move {
            Ok(self.tx.query_opt("SELECT unit_id FROM loan.device_units WHERE model_id = $1 AND status = 'AVAILABLE' ORDER BY unit_id LIMIT 1 FOR UPDATE", &[&model]).await.map_err(pg_error)?.map(|r| r.get(0)))
        })
    }
    fn lock_reservation(
        &mut self,
        id: Uuid,
    ) -> BoxFuture<'_, Result<(LockedModel, Reservation), ApplicationError>> {
        Box::pin(async move {
            // The model reference is immutable. No decision uses this unlocked read.
            let row = self
                .tx
                .query_opt(
                    "SELECT model_id FROM loan.reservations WHERE reservation_id = $1",
                    &[&id],
                )
                .await
                .map_err(pg_error)?
                .ok_or_else(|| ApplicationError::NotFound(format!("reservation {id}")))?;
            let model = self.lock_model(row.get(0)).await?;
            let row = self
                .tx
                .query_opt(
                    "SELECT * FROM loan.reservations WHERE reservation_id = $1 FOR UPDATE",
                    &[&id],
                )
                .await
                .map_err(pg_error)?
                .ok_or_else(|| ApplicationError::NotFound(format!("reservation {id}")))?;
            let unit: String = row.get("unit_id");
            self.tx
                .query_one(
                    "SELECT unit_id FROM loan.device_units WHERE unit_id=$1 FOR UPDATE",
                    &[&unit],
                )
                .await
                .map_err(pg_error)?;
            Ok((model, reservation_from_row(&row)))
        })
    }
    fn insert_reservation(
        &mut self,
        r: Reservation,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            self.tx.execute("UPDATE loan.device_units SET status = 'RESERVED', updated_at = $2 WHERE unit_id = $1", &[&r.unit_id, &r.reserved_at]).await.map_err(pg_error)?;
            self.tx.execute("UPDATE loan.device_models SET available_units = available_units - 1, updated_at = $2 WHERE id = $1", &[&r.model_id, &r.reserved_at]).await.map_err(pg_error)?;
            self.tx.execute("INSERT INTO loan.reservations (reservation_id, model_id, unit_id, student_sub, student_email, status, reserved_at, due_at, correlation_id) VALUES ($1,$2,$3,$4,$5,'RESERVED',$6,$7,$8)", &[&r.reservation_id, &r.model_id, &r.unit_id, &r.student_sub, &r.student_email, &r.reserved_at, &r.due_at, &r.correlation_id]).await.map_err(pg_error)?;
            Ok(())
        })
    }
    fn upsert_subscription(
        &mut self,
        actor: AuthenticatedActor,
        receipt: SubscriptionReceipt,
    ) -> BoxFuture<'_, Result<SubscriptionReceipt, ApplicationError>> {
        Box::pin(async move {
            let row = self.tx.query_one("INSERT INTO loan.availability_subscriptions (subscription_id, model_id, student_sub, student_email, status, created_at) VALUES ($1,$2,$3,$4,'ACTIVE',$5) ON CONFLICT (model_id, student_sub) WHERE status = 'ACTIVE' DO UPDATE SET student_email = EXCLUDED.student_email RETURNING subscription_id, created_at", &[&receipt.subscription_id, &receipt.model_id, &actor.subject, &actor.email, &receipt.created_at]).await.map_err(pg_error)?;
            Ok(SubscriptionReceipt {
                subscription_id: row.get(0),
                created_at: row.get(1),
                ..receipt
            })
        })
    }
    fn save_collection(&mut self, r: Reservation) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            self.tx.execute("UPDATE loan.reservations SET status = $2, collected_at = $3 WHERE reservation_id = $1", &[&r.reservation_id, &r.status, &r.collected_at]).await.map_err(pg_error)?;
            self.tx.execute("UPDATE loan.device_units SET status = 'COLLECTED', updated_at = $2 WHERE unit_id = $1", &[&r.unit_id, &r.collected_at]).await.map_err(pg_error)?;
            Ok(())
        })
    }
    fn active_subscribers(
        &mut self,
        model: String,
    ) -> BoxFuture<'_, Result<Vec<String>, ApplicationError>> {
        Box::pin(async move {
            let rows = self.tx.query("SELECT student_email FROM loan.availability_subscriptions WHERE model_id = $1 AND status = 'ACTIVE' ORDER BY subscription_id FOR UPDATE", &[&model]).await.map_err(pg_error)?;
            Ok(rows.iter().map(|r| r.get(0)).collect())
        })
    }
    fn save_return(&mut self, r: Reservation) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            self.tx.execute("UPDATE loan.reservations SET status = $2, returned_at = $3 WHERE reservation_id = $1", &[&r.reservation_id, &r.status, &r.returned_at]).await.map_err(pg_error)?;
            self.tx.execute("UPDATE loan.device_units SET status = 'AVAILABLE', updated_at = $2 WHERE unit_id = $1", &[&r.unit_id, &r.returned_at]).await.map_err(pg_error)?;
            self.tx.execute("UPDATE loan.device_models SET available_units = available_units + 1, updated_at = $2 WHERE id = $1", &[&r.model_id, &r.returned_at]).await.map_err(pg_error)?;
            self.tx.execute("UPDATE loan.availability_subscriptions SET status = 'NOTIFIED', notified_at = $2 WHERE model_id = $1 AND status = 'ACTIVE'", &[&r.model_id, &r.returned_at]).await.map_err(pg_error)?;
            Ok(())
        })
    }
    fn append_outbox(
        &mut self,
        events: Vec<NotificationRequestV1>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            for e in events {
                let payload = serde_json::to_value(&e)
                    .map_err(|e| ApplicationError::Persistence(e.to_string()))?;
                self.tx.execute("INSERT INTO loan.outbox_events (event_id,event_type,contract_version,occurred_at,correlation_id,payload) VALUES ($1,$2,$3,$4,$5,$6) ON CONFLICT (event_id) DO NOTHING", &[&e.event_id, &e.event_type, &(e.version as i32), &e.occurred_at, &e.correlation_id, &payload]).await.map_err(pg_error)?;
            }
            Ok(())
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
                return Err(ApplicationError::Broker(format!(
                    "servicebus rejected send ({status})"
                )));
            }
            Ok(())
        })
    }
}

pub use crate::auth::{Auth0JwtVerifier, AuthVerifyError};
