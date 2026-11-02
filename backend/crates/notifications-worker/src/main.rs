pub mod application;
#[cfg(test)]
mod cosmos_tests;
pub mod domain;
pub mod infrastructure;

use application::{
    CreateJobOutcome, MessageDisposition, NotificationJobStorePort, NotificationProcessor,
    ReplaceJobOutcome,
};
use chrono::Utc;
use domain::NotificationJobDocument;
use infrastructure::{
    verify_postgres_access_denied, ConfigurableEmailSender, CosmosRestJobStore,
    KeyVaultSecretClient, SecretLookupOutcome, ServiceBusRestConsumer,
};
use rc_shared::{
    init_json_tracing, ManagedIdentityTokenProvider, NotificationRequestV1, CONTRACT_VERSION_V1,
    EVENT_TYPE_AVAILABILITY_REQUESTED,
};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_json_tracing("notifications-worker");

    let mode = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("NOTIFICATIONS_RUN_MODE").ok())
        .unwrap_or_else(|| "worker".to_string());

    let client_id = std::env::var("AZURE_CLIENT_ID").unwrap_or_default();
    let token_provider = Arc::new(ManagedIdentityTokenProvider::new(client_id)?);

    let cosmos_endpoint =
        std::env::var("COSMOS_ENDPOINT").expect("COSMOS_ENDPOINT environment variable is required");
    let cosmos_database =
        std::env::var("COSMOS_DATABASE").unwrap_or_else(|_| "notifications".to_string());
    let cosmos_container = std::env::var("COSMOS_CONTAINER").unwrap_or_else(|_| "jobs".to_string());

    let store = Arc::new(CosmosRestJobStore::new(
        cosmos_endpoint,
        cosmos_database,
        cosmos_container,
        Arc::clone(&token_provider),
    ));

    match mode.as_str() {
        "worker" => run_worker(store, token_provider, false).await?,
        "drain-once" => run_worker(store, token_provider, true).await?,
        "verify-isolation" => run_verify_isolation(store, token_provider).await?,
        other => {
            return Err(format!(
                "unknown notifications-worker mode '{other}' (expected: worker | drain-once | verify-isolation)"
            )
            .into());
        }
    }

    Ok(())
}

async fn run_worker(
    store: Arc<CosmosRestJobStore>,
    token_provider: Arc<ManagedIdentityTokenProvider>,
    drain_once: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let sb_namespace = std::env::var("SERVICEBUS_NAMESPACE_FQDN")
        .expect("SERVICEBUS_NAMESPACE_FQDN environment variable is required");
    let sb_queue = std::env::var("SERVICEBUS_QUEUE_NAME")
        .unwrap_or_else(|_| "notification-requests-v1".to_string());

    let consumer = Arc::new(ServiceBusRestConsumer::new(
        sb_namespace.clone(),
        sb_queue.clone(),
        Arc::clone(&token_provider),
    ));

    let keyvault_client = std::env::var("KEYVAULT_URI")
        .ok()
        .filter(|v| !v.is_empty())
        .map(|uri| KeyVaultSecretClient::new(uri, Arc::clone(&token_provider)));

    if let Some(ref kv) = keyvault_client {
        let secret_name =
            std::env::var("RESEND_SECRET_NAME").unwrap_or_else(|_| "resend-api-key".to_string());
        match kv.get_secret(&secret_name).await {
            Ok(SecretLookupOutcome::Found(_)) => {
                info!(secret_name = %secret_name, "verified Key Vault reachability (secret present)");
            }
            Ok(SecretLookupOutcome::NotFound) => {
                info!(secret_name = %secret_name, "verified Key Vault RBAC reachability (secret not populated; FakeEmailSender active)");
            }
            Err(err) => {
                warn!(error = %err, "Key Vault reachability check returned warning during startup");
            }
        }
    }

    let delivery_mode = std::env::var("EMAIL_DELIVERY_MODE").unwrap_or_else(|_| "fake".to_string());
    let secret_name =
        std::env::var("RESEND_SECRET_NAME").unwrap_or_else(|_| "resend-api-key".to_string());
    let from_address = std::env::var("RESEND_FROM_ADDRESS")
        .unwrap_or_else(|_| "Resource Center <noreply@resource.center.example>".to_string());
    let authorized_recipients: HashSet<String> = std::env::var("RESEND_AUTHORIZED_RECIPIENTS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let sender = ConfigurableEmailSender::new(
        delivery_mode.clone(),
        keyvault_client,
        secret_name,
        from_address,
        authorized_recipients,
    );

    let max_attempts: u32 = std::env::var("MAX_DELIVERY_ATTEMPTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);

    let processor = NotificationProcessor::new(store.as_ref(), sender, max_attempts);

    info!(
        servicebus_namespace = %sb_namespace,
        queue = %sb_queue,
        delivery_mode = %delivery_mode,
        drain_once = drain_once,
        "starting notifications-worker loop"
    );

    let mut processed_count = 0usize;
    loop {
        let wait_secs = if drain_once { 5 } else { 20 };
        let receive_fut = consumer.receive_peek_lock(wait_secs);

        let maybe_msg = tokio::select! {
            _ = shutdown_signal() => {
                info!(processed_count = processed_count, "shutdown signal received while waiting for Service Bus message; exiting cleanly");
                break;
            }
            res = receive_fut => {
                match res {
                    Ok(m) => m,
                    Err(err) => {
                        error!(error = %err, "error polling Service Bus queue");
                        if drain_once {
                            return Err(err.into());
                        }
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                }
            }
        };

        let Some(msg) = maybe_msg else {
            if drain_once {
                info!(
                    processed_count = processed_count,
                    "queue returned 204 No Content in drain-once mode; exiting"
                );
                break;
            }
            continue;
        };

        // Keep retries on this locked delivery, so waiting does not consume the
        // broker delivery budget. Dropping either future cancels in-flight work.
        let processing = processor.process_until_settled(&msg.body);
        let renewals = async {
            loop {
                tokio::time::sleep(Duration::from_secs(10)).await;
                match tokio::time::timeout(
                    Duration::from_secs(10),
                    consumer.renew_lock(&msg.lock_location),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    _ => break,
                }
            }
        };
        let result = tokio::select! {
            biased;
            _ = shutdown_signal() => {
                // Retain the message and Cosmos lease for recovery after expiry.
                info!("shutdown: cancelled processing; retained work will recover");
                break;
            }
            _ = renewals => {
                warn!("broker lock renewal failed; cancelled processing");
                continue;
            }
            result = processing => result,
        };

        match result {
            MessageDisposition::Complete {
                event_id,
                duplicate_skipped,
                final_status,
            } => {
                consumer.complete_message(&msg.lock_location).await?;
                processed_count += 1;
                info!(
                    event_id = %event_id,
                    duplicate_skipped = duplicate_skipped,
                    final_status = ?final_status,
                    delivery_count = ?msg.delivery_count,
                    "settled (completed) Service Bus notification message"
                );
            }
            MessageDisposition::Wait { .. } => {
                unreachable!("wait handled inside processing loop")
            }
            MessageDisposition::Abandon { event_id, reason } => {
                warn!(
                    event_id = %event_id,
                    reason = %reason,
                    delivery_count = ?msg.delivery_count,
                    "abandoning Service Bus message for retry or dead-letter"
                );
                let _ = consumer.abandon_message(&msg.lock_location).await;
            }
        }
    }

    Ok(())
}

async fn run_verify_isolation(
    store: Arc<CosmosRestJobStore>,
    token_provider: Arc<ManagedIdentityTokenProvider>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Verify Key Vault reachability (200 or 404 expected)
    let kv_uri = std::env::var("KEYVAULT_URI").expect("KEYVAULT_URI required for verify-isolation");
    let kv = KeyVaultSecretClient::new(kv_uri, Arc::clone(&token_provider));
    let kv_outcome = kv.get_secret("resend-api-key").await?;
    let kv_status = match kv_outcome {
        SecretLookupOutcome::Found(_) => "200_found",
        SecretLookupOutcome::NotFound => "404_not_found_rbac_allowed",
    };

    // 2. Verify Cosmos DB create + 409 conflict + ETag conditional replace + stale ETag 412 + cleanup
    let event_id = format!("evt-isolation-{}", uuid::Uuid::new_v4());
    let req = NotificationRequestV1 {
        event_id: event_id.clone(),
        event_type: EVENT_TYPE_AVAILABILITY_REQUESTED.to_string(),
        version: CONTRACT_VERSION_V1,
        occurred_at: Utc::now(),
        correlation_id: format!("corr-{event_id}"),
        recipient: "probe@example.edu".to_string(),
        device_model: "Isolation Probe Device".to_string(),
        synthetic_probe: Some(true),
    };
    let doc = NotificationJobDocument::new_claimed(&req, "fake", Utc::now());
    let created = match store.create_job(&doc).await? {
        CreateJobOutcome::Created(v) => v,
        CreateJobOutcome::AlreadyExists => return Err("unexpected conflict on fresh UUID".into()),
    };
    let conflict = store.create_job(&doc).await?;
    assert_eq!(conflict, CreateJobOutcome::AlreadyExists);

    let mut delivered_doc = created.document.clone();
    delivered_doc.mark_delivered("fake-isolation".to_string(), "fake", Utc::now());
    let replaced = match store.replace_job(&delivered_doc, &created.etag).await? {
        ReplaceJobOutcome::Replaced(v) => v,
        ReplaceJobOutcome::PreconditionFailed => {
            return Err("unexpected 412 with valid etag".into());
        }
    };
    // Stale ETag must return PreconditionFailed (412)
    let stale = store.replace_job(&replaced.document, &created.etag).await?;
    assert_eq!(stale, ReplaceJobOutcome::PreconditionFailed);

    store.delete_job(&event_id).await?;

    // 3. Verify PostgreSQL rejects the notifications managed identity
    let pg_host = std::env::var("PGHOST").expect("PGHOST required for verify-isolation");
    let pg_port: u16 = std::env::var("PGPORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5432);
    let pg_db = std::env::var("PGDATABASE").unwrap_or_else(|_| "resource_center".to_string());
    let pg_user =
        std::env::var("PGUSER").unwrap_or_else(|_| "rc-test-0330980f-notifications".to_string());

    let pg_denial =
        verify_postgres_access_denied(&pg_host, pg_port, &pg_db, &pg_user, &token_provider).await?;

    info!(
        keyvault_check = kv_status,
        cosmos_etag_conflict_check = "passed_and_cleaned",
        postgres_denial = %pg_denial,
        "notifications-worker verify-isolation succeeded"
    );
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sigterm) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
