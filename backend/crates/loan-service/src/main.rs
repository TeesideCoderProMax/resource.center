mod application;
mod auth;
mod domain;
mod infrastructure;
mod presentation;

use std::net::SocketAddr;
use std::sync::Arc;

use rc_shared::{init_json_tracing, ManagedIdentityTokenProvider};

use crate::application::run_outbox_publish_batch;
use crate::infrastructure::{
    Auth0JwtVerifier, PostgresAdapter, PostgresConfig, ServiceBusRestPublisher,
};
use crate::presentation::{build_router, AppState};

#[tokio::main]
async fn main() {
    let mode = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("SERVICE_MODE").ok())
        .unwrap_or_else(|| "api".to_string());

    init_json_tracing(&format!("loan-{mode}"));

    if let Err(err) = run_mode(&mode).await {
        tracing::error!(mode = %mode, error = %err, "loan-service exited with error");
        std::process::exit(1);
    }
}

async fn run_mode(mode: &str) -> Result<(), String> {
    let pg_adapter = build_postgres_adapter()?;

    match mode {
        "api" => run_api_server(pg_adapter).await,
        "migrate" => {
            let seed_demo = std::env::var("SEED_DEMO_DATA")
                .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
                .unwrap_or(false);
            let started = std::time::Instant::now();
            pg_adapter
                .run_migrations(seed_demo)
                .await
                .map_err(|e| e.to_string())?;
            tracing::info!(
                seed_demo = seed_demo,
                duration_ms = started.elapsed().as_millis(),
                "database schema migrations completed"
            );
            Ok(())
        }
        "publish" => {
            let batch_size: i64 = std::env::var("OUTBOX_BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(25);
            let lease_seconds: i64 = std::env::var("OUTBOX_LEASE_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60);

            let client_id = std::env::var("AZURE_CLIENT_ID")
                .map_err(|_| "AZURE_CLIENT_ID is required for publisher mode".to_string())?;
            let token_provider =
                ManagedIdentityTokenProvider::new(client_id).map_err(|e| e.to_string())?;
            let namespace = std::env::var("SERVICEBUS_NAMESPACE")
                .map_err(|_| "SERVICEBUS_NAMESPACE is required for publisher mode".to_string())?;
            let queue = std::env::var("SERVICEBUS_QUEUE")
                .unwrap_or_else(|_| "notification-requests-v1".to_string());
            let publisher = ServiceBusRestPublisher::new(namespace, queue, token_provider)
                .map_err(|e| e.to_string())?;

            let summary =
                run_outbox_publish_batch(&pg_adapter, &publisher, batch_size, lease_seconds)
                    .await
                    .map_err(|e| e.to_string())?;

            tracing::info!(
                claimed_count = summary.claimed_count,
                published_count = summary.published_count,
                failed_count = summary.failed_count,
                duration_ms = summary.duration_ms,
                "outbox publisher batch finished"
            );
            if summary.failed_count > 0 {
                return Err(format!(
                    "{} outbox message(s) failed broker publication",
                    summary.failed_count
                ));
            }
            Ok(())
        }
        "enqueue-probe" => {
            let event_id = std::env::var("PROBE_EVENT_ID")
                .unwrap_or_else(|_| format!("evt-probe-{}", uuid::Uuid::new_v4()));
            let correlation_id = std::env::var("PROBE_CORRELATION_ID")
                .unwrap_or_else(|_| format!("corr-probe-{}", uuid::Uuid::new_v4()));
            pg_adapter
                .enqueue_synthetic_outbox_probe(&event_id, &correlation_id)
                .await
                .map_err(|e| e.to_string())?;
            tracing::info!(
                event_id = %event_id,
                correlation_id = %correlation_id,
                "enqueued synthetic outbox probe row in loan.outbox_events"
            );
            Ok(())
        }
        "verify-resilience" => {
            let report = pg_adapter
                .run_resilience_suite()
                .await
                .map_err(|e| e.to_string())?;
            tracing::info!(
                report = %serde_json::to_string(&report).unwrap_or_default(),
                "completed live PostgreSQL concurrency, outbox atomicity, and publisher crash-window suite"
            );
            Ok(())
        }
        other => Err(format!(
            "unknown SERVICE_MODE '{other}'; expected api, migrate, publish, enqueue-probe, or verify-resilience"
        )),
    }
}

fn build_postgres_adapter() -> Result<PostgresAdapter, String> {
    let host = std::env::var("PGHOST").map_err(|_| "PGHOST is required".to_string())?;
    let port: u16 = std::env::var("PGPORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5432);
    let database = std::env::var("PGDATABASE").unwrap_or_else(|_| "loan".to_string());
    let user = std::env::var("PGUSER").map_err(|_| "PGUSER is required".to_string())?;
    let static_password = std::env::var("PGPASSWORD")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let max_connections: usize = std::env::var("PG_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let publisher_role_name = std::env::var("PUBLISHER_PGUSER")
        .ok()
        .filter(|s| !s.trim().is_empty());

    let token_provider = match std::env::var("AZURE_CLIENT_ID") {
        Ok(cid) if !cid.trim().is_empty() => {
            Some(ManagedIdentityTokenProvider::new(cid).map_err(|e| e.to_string())?)
        }
        _ => None,
    };

    Ok(PostgresAdapter::new(
        PostgresConfig {
            host,
            port,
            database,
            user,
            static_password,
            max_connections,
            publisher_role_name,
        },
        token_provider,
    ))
}

async fn run_api_server(pg_adapter: PostgresAdapter) -> Result<(), String> {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);
    let environment = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "test".to_string());
    let service_version =
        std::env::var("SERVICE_VERSION").unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    let allowed_origins: Vec<String> = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    let state = AppState {
        repo: Arc::new(pg_adapter),
        auth: Auth0JwtVerifier::from_env(),
        environment,
        service_version,
    };

    let app = build_router(state, &allowed_origins);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("failed to bind {addr}: {e}"))?;

    tracing::info!(%addr, "loan-api listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| format!("axum server error: {e}"))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut sig) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            sig.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received SIGINT, initiating graceful shutdown"),
        _ = terminate => tracing::info!("received SIGTERM, initiating graceful shutdown"),
    }
}

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod postgres_tests;
