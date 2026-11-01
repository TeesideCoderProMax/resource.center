use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowOrigin, CorsLayer};
use uuid::Uuid;

use crate::application::{ApplicationError, LoanRepositoryPort};
use crate::domain::DomainError;
use crate::infrastructure::{Auth0JwtVerifier, AuthVerifyError};

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn LoanRepositoryPort>,
    pub auth: Auth0JwtVerifier,
    pub environment: String,
    pub service_version: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
    pub environment: String,
    pub version: String,
    pub auth0_configured: bool,
    pub correlation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiErrorResponse {
    pub error: &'static str,
    pub message: String,
    pub correlation_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReservationRequest {
    pub model_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubscriptionRequest {
    pub model_id: String,
}

pub fn build_router(state: AppState, allowed_origins: &[String]) -> Router {
    let cors = build_cors_layer(allowed_origins);

    Router::new()
        .route("/health/startup", get(health_startup))
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/api/v1/catalogue", get(list_catalogue))
        .route("/api/v1/reservations", post(create_reservation))
        .route("/api/v1/reservations/{id}/collect", post(record_collection))
        .route("/api/v1/reservations/{id}/return", post(record_return))
        .route("/api/v1/subscriptions", post(create_subscription))
        .layer(cors)
        .with_state(state)
}

fn build_cors_layer(allowed_origins: &[String]) -> CorsLayer {
    let parsed_origins: Vec<HeaderValue> = allowed_origins
        .iter()
        .filter_map(|o| HeaderValue::from_str(o.trim()).ok())
        .collect();

    let origin_policy = if parsed_origins.is_empty() {
        AllowOrigin::any()
    } else {
        AllowOrigin::list(parsed_origins)
    };

    CorsLayer::new()
        .allow_origin(origin_policy)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderName::from_static("x-correlation-id"),
        ])
        .expose_headers([axum::http::HeaderName::from_static("x-correlation-id")])
}

fn extract_correlation_id(headers: &HeaderMap) -> String {
    headers
        .get("x-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("corr-{}", Uuid::new_v4()))
}

fn with_correlation_header(
    status: StatusCode,
    correlation_id: &str,
    body: impl Serialize,
) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    if let Ok(val) = HeaderValue::from_str(correlation_id) {
        headers.insert("x-correlation-id", val);
    }
    (status, headers, Json(body))
}

async fn health_startup(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    with_correlation_header(
        StatusCode::OK,
        &correlation_id,
        HealthResponse {
            status: "ok",
            service: "loan-api",
            environment: state.environment,
            version: state.service_version,
            auth0_configured: state.auth.is_configured(),
            correlation_id: correlation_id.clone(),
            database: None,
        },
    )
}

async fn health_live(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    with_correlation_header(
        StatusCode::OK,
        &correlation_id,
        HealthResponse {
            status: "ok",
            service: "loan-api",
            environment: state.environment,
            version: state.service_version,
            auth0_configured: state.auth.is_configured(),
            correlation_id: correlation_id.clone(),
            database: None,
        },
    )
}

async fn health_ready(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    match state.repo.check_readiness().await {
        Ok(()) => with_correlation_header(
            StatusCode::OK,
            &correlation_id,
            HealthResponse {
                status: "ready",
                service: "loan-api",
                environment: state.environment,
                version: state.service_version,
                auth0_configured: state.auth.is_configured(),
                correlation_id: correlation_id.clone(),
                database: Some("reachable"),
            },
        )
        .into_response(),
        Err(err) => {
            tracing::error!(
                correlation_id = %correlation_id,
                error = %err,
                "loan-api readiness check failed"
            );
            with_correlation_header(
                StatusCode::SERVICE_UNAVAILABLE,
                &correlation_id,
                ApiErrorResponse {
                    error: "DatabaseUnreachable",
                    message: "Loan PostgreSQL readiness check failed".to_string(),
                    correlation_id: correlation_id.clone(),
                },
            )
            .into_response()
        }
    }
}

async fn list_catalogue(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    match state.repo.list_catalogue().await {
        Ok(items) => {
            tracing::info!(
                correlation_id = %correlation_id,
                item_count = items.len(),
                "served public device catalogue"
            );
            with_correlation_header(
                StatusCode::OK,
                &correlation_id,
                serde_json::json!({
                    "items": items,
                    "correlationId": correlation_id,
                    "auth0Configured": state.auth.is_configured(),
                }),
            )
            .into_response()
        }
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn create_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateReservationRequest>,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(&state.auth, &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    if let Err(err) = crate::application::validate_reserve_request(&actor, &payload.model_id) {
        return map_app_error(ApplicationError::Domain(err), &correlation_id).into_response();
    }

    match state
        .repo
        .reserve_device(actor, payload.model_id, correlation_id.clone())
        .await
    {
        Ok(receipt) => {
            tracing::info!(
                correlation_id = %correlation_id,
                reservation_id = %receipt.reservation_id,
                unit_id = %receipt.unit_id,
                "device reservation committed"
            );
            with_correlation_header(StatusCode::CREATED, &correlation_id, receipt).into_response()
        }
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn create_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSubscriptionRequest>,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(&state.auth, &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    if let Err(err) = crate::application::validate_reserve_request(&actor, &payload.model_id) {
        return map_app_error(ApplicationError::Domain(err), &correlation_id).into_response();
    }

    match state
        .repo
        .subscribe_availability(actor, payload.model_id)
        .await
    {
        Ok(receipt) => {
            tracing::info!(
                correlation_id = %correlation_id,
                subscription_id = %receipt.subscription_id,
                "availability subscription committed"
            );
            with_correlation_header(StatusCode::CREATED, &correlation_id, receipt).into_response()
        }
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn record_collection(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(&state.auth, &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    if let Err(err) = crate::application::validate_staff_action(&actor) {
        return map_app_error(ApplicationError::Domain(err), &correlation_id).into_response();
    }

    match state
        .repo
        .record_collection(actor, id, correlation_id.clone())
        .await
    {
        Ok(receipt) => {
            tracing::info!(
                correlation_id = %correlation_id,
                reservation_id = %receipt.reservation_id,
                outbox_event_id = %receipt.outbox_event_id,
                "device collection and outbox event committed atomically"
            );
            with_correlation_header(StatusCode::OK, &correlation_id, receipt).into_response()
        }
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn record_return(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(&state.auth, &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    if let Err(err) = crate::application::validate_staff_action(&actor) {
        return map_app_error(ApplicationError::Domain(err), &correlation_id).into_response();
    }

    match state
        .repo
        .record_return(actor, id, correlation_id.clone())
        .await
    {
        Ok(receipt) => {
            tracing::info!(
                correlation_id = %correlation_id,
                reservation_id = %receipt.reservation_id,
                outbox_count = receipt.outbox_event_ids.len(),
                "device return and outbox events committed atomically"
            );
            with_correlation_header(StatusCode::OK, &correlation_id, receipt).into_response()
        }
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn authenticate_request(
    verifier: &Auth0JwtVerifier,
    headers: &HeaderMap,
) -> Result<crate::domain::AuthenticatedActor, AuthVerifyError> {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    verifier.verify_bearer(auth_header).await
}

fn map_auth_error(err: AuthVerifyError, correlation_id: &str) -> impl IntoResponse {
    let (status, code) = match err {
        AuthVerifyError::NotConfigured => (StatusCode::SERVICE_UNAVAILABLE, "AuthNotConfigured"),
        AuthVerifyError::MissingBearerToken => (StatusCode::UNAUTHORIZED, "Unauthorized"),
        AuthVerifyError::InvalidToken(_) => (StatusCode::UNAUTHORIZED, "InvalidToken"),
    };
    with_correlation_header(
        status,
        correlation_id,
        ApiErrorResponse {
            error: code,
            message: err.to_string(),
            correlation_id: correlation_id.to_string(),
        },
    )
}

fn map_app_error(err: ApplicationError, correlation_id: &str) -> impl IntoResponse {
    let (status, code) = match &err {
        ApplicationError::Domain(DomainError::ForbiddenRole(_)) => {
            (StatusCode::FORBIDDEN, "ForbiddenRole")
        }
        ApplicationError::Domain(DomainError::NoAvailableUnits(_)) => {
            (StatusCode::CONFLICT, "NoAvailableUnits")
        }
        ApplicationError::Domain(DomainError::UnitsCurrentlyAvailable(_)) => {
            (StatusCode::CONFLICT, "UnitsCurrentlyAvailable")
        }
        ApplicationError::Domain(DomainError::InvalidTransition { .. }) => {
            (StatusCode::CONFLICT, "InvalidTransition")
        }
        ApplicationError::Domain(DomainError::InvalidInput(_)) => {
            (StatusCode::BAD_REQUEST, "InvalidInput")
        }
        ApplicationError::NotFound(_) => (StatusCode::NOT_FOUND, "NotFound"),
        ApplicationError::Persistence(_) | ApplicationError::Broker(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "InternalError")
        }
    };
    tracing::warn!(
        correlation_id = %correlation_id,
        error_code = code,
        error = %err,
        "request failed"
    );
    with_correlation_header(
        status,
        correlation_id,
        ApiErrorResponse {
            error: code,
            message: err.to_string(),
            correlation_id: correlation_id.to_string(),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        build_collection_outbox_event, build_return_outbox_events, BoxFuture, CollectionReceipt,
        ReservationReceipt, ReturnReceipt, SubscriptionReceipt,
    };
    use crate::domain::{
        compute_two_day_due_at, ensure_staff, ensure_student_or_staff, AuthenticatedActor,
        DeviceModelSummary, DomainError,
    };
    use axum::body::Body;
    use axum::http::Request;
    use chrono::Utc;
    use std::sync::Mutex;
    use tower::ServiceExt;

    #[derive(Default)]
    struct StatefulFakeLoanRepo {
        available_units: Mutex<i32>,
        subscriptions: Mutex<Vec<String>>,
        outbox_events: Mutex<Vec<rc_shared::NotificationRequestV1>>,
    }

    impl StatefulFakeLoanRepo {
        fn with_units(units: i32) -> Self {
            Self {
                available_units: Mutex::new(units),
                subscriptions: Mutex::new(Vec::new()),
                outbox_events: Mutex::new(Vec::new()),
            }
        }
    }

    impl LoanRepositoryPort for StatefulFakeLoanRepo {
        fn check_readiness(&self) -> BoxFuture<'_, Result<(), ApplicationError>> {
            Box::pin(async { Ok(()) })
        }

        fn list_catalogue(
            &self,
        ) -> BoxFuture<'_, Result<Vec<DeviceModelSummary>, ApplicationError>> {
            let avail = *self.available_units.lock().unwrap();
            Box::pin(async move {
                Ok(vec![DeviceModelSummary {
                    id: "dell-latitude-7440".into(),
                    name: "Dell Latitude 7440".into(),
                    category: "Laptop".into(),
                    description: "2-day loan laptop".into(),
                    loan_duration_days: 2,
                    total_units: 2,
                    available_units: avail,
                }])
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
                let mut avail = self.available_units.lock().unwrap();
                if *avail <= 0 {
                    return Err(ApplicationError::Domain(DomainError::NoAvailableUnits(
                        model_id,
                    )));
                }
                *avail -= 1;
                Ok(ReservationReceipt {
                    reservation_id: Uuid::new_v4(),
                    model_id,
                    unit_id: "unit-lat-001".into(),
                    status: "RESERVED".into(),
                    reserved_at: Utc::now(),
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
                let avail = *self.available_units.lock().unwrap();
                if avail > 0 {
                    return Err(ApplicationError::Domain(
                        DomainError::UnitsCurrentlyAvailable(model_id),
                    ));
                }
                self.subscriptions.lock().unwrap().push(actor.email);
                Ok(SubscriptionReceipt {
                    subscription_id: Uuid::new_v4(),
                    model_id,
                    status: "ACTIVE".into(),
                    created_at: Utc::now(),
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
                let now = Utc::now();
                let due_at = compute_two_day_due_at(now);
                let evt = build_collection_outbox_event(
                    reservation_id,
                    "borrower@tees.ac.uk",
                    "Dell Latitude 7440",
                    now,
                    &correlation_id,
                );
                let evt_id = evt.event_id.clone();
                self.outbox_events.lock().unwrap().push(evt);
                Ok(CollectionReceipt {
                    reservation_id,
                    unit_id: "unit-lat-001".into(),
                    status: "COLLECTED".into(),
                    collected_at: now,
                    due_at,
                    outbox_event_id: evt_id,
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
                let now = Utc::now();
                *self.available_units.lock().unwrap() += 1;
                let subs: Vec<String> = self.subscriptions.lock().unwrap().drain(..).collect();
                let evts = build_return_outbox_events(
                    reservation_id,
                    "borrower@tees.ac.uk",
                    &subs,
                    "Dell Latitude 7440",
                    now,
                    &correlation_id,
                );
                let ids: Vec<String> = evts.iter().map(|e| e.event_id.clone()).collect();
                self.outbox_events.lock().unwrap().extend(evts);
                Ok(ReturnReceipt {
                    reservation_id,
                    unit_id: "unit-lat-001".into(),
                    model_id: "dell-latitude-7440".into(),
                    status: "RETURNED".into(),
                    returned_at: now,
                    outbox_event_ids: ids,
                })
            })
        }
    }

    #[tokio::test]
    async fn health_and_catalogue_endpoints_respond_with_correlation_id() {
        let state = AppState {
            repo: Arc::new(StatefulFakeLoanRepo::with_units(2)),
            auth: Auth0JwtVerifier::from_env(),
            environment: "test".into(),
            service_version: "0.1.0-test".into(),
        };
        let app = build_router(state, &["https://example.edu".into()]);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .header("x-correlation-id", "corr-test-99")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("x-correlation-id").unwrap(),
            "corr-test-99"
        );

        let cat_resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/catalogue")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cat_resp.status(), StatusCode::OK);

        // Protected route without Auth0 env configured returns 503 AuthNotConfigured
        let res_resp = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/v1/reservations")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"modelId":"dell-latitude-7440"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn concurrent_last_unit_reservation_and_three_notification_flows_work() {
        let repo = Arc::new(StatefulFakeLoanRepo::with_units(1));

        // 5 concurrent reservation contenders for the 1 available unit
        let mut handles = Vec::new();
        for i in 0..5 {
            let r = Arc::clone(&repo);
            handles.push(tokio::spawn(async move {
                let actor = AuthenticatedActor {
                    subject: format!("auth0|student-{i}"),
                    email: format!("student{i}@tees.ac.uk"),
                    roles: vec![crate::domain::Role::Student],
                };
                r.reserve_device(actor, "dell-latitude-7440".into(), format!("corr-{i}"))
                    .await
            }));
        }

        let mut winners = Vec::new();
        let mut conflicts = 0usize;
        for h in handles {
            match h.await.unwrap() {
                Ok(receipt) => winners.push(receipt),
                Err(ApplicationError::Domain(DomainError::NoAvailableUnits(_))) => conflicts += 1,
                Err(other) => panic!("unexpected error: {other}"),
            }
        }
        assert_eq!(winners.len(), 1);
        assert_eq!(conflicts, 4);

        let res_id = winners[0].reservation_id;

        // Subscribe waitlisted student while 0 units available
        let waiter = AuthenticatedActor {
            subject: "auth0|waiter".into(),
            email: "waiter@tees.ac.uk".into(),
            roles: vec![crate::domain::Role::Student],
        };
        let sub = repo
            .subscribe_availability(waiter.clone(), "dell-latitude-7440".into())
            .await
            .unwrap();
        assert_eq!(sub.status, "ACTIVE");

        // Student attempting staff collection is rejected with ForbiddenRole("staff") -> 403
        let forbidden = repo
            .record_collection(waiter, res_id, "corr-forbidden".into())
            .await
            .unwrap_err();
        let forbidden_resp = map_app_error(forbidden, "corr-forbidden").into_response();
        assert_eq!(forbidden_resp.status(), StatusCode::FORBIDDEN);

        // Staff records collection -> 2-day due date + collection_recorded outbox event
        let staff = AuthenticatedActor {
            subject: "auth0|staff-1".into(),
            email: "staff@tees.ac.uk".into(),
            roles: vec![crate::domain::Role::Staff],
        };
        let collected = repo
            .record_collection(staff.clone(), res_id, "corr-collect".into())
            .await
            .unwrap();
        assert_eq!((collected.due_at - collected.collected_at).num_hours(), 48);

        // Staff records return -> restores availability + emits return_recorded & availability_notification_requested
        let returned = repo
            .record_return(staff, res_id, "corr-return".into())
            .await
            .unwrap();
        assert_eq!(returned.outbox_event_ids.len(), 2);

        let outbox = repo.outbox_events.lock().unwrap();
        assert_eq!(outbox.len(), 3);
        assert_eq!(
            outbox[0].event_type,
            rc_shared::EVENT_TYPE_COLLECTION_RECORDED
        );
        assert_eq!(outbox[1].event_type, rc_shared::EVENT_TYPE_RETURN_RECORDED);
        assert_eq!(
            outbox[2].event_type,
            rc_shared::EVENT_TYPE_AVAILABILITY_REQUESTED
        );
    }
}
