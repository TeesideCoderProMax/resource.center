use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tower_http::cors::{AllowOrigin, CorsLayer};
use uuid::Uuid;

use crate::application::{
    AccessTokenVerifierPort, ApplicationError, AuthVerifyError, LoanRepositoryPort,
};
use crate::domain::{CreateDeviceModelInput, DomainError, NewDeviceUnitInput};

#[derive(Clone)]
pub struct AppState {
    pub repo: Arc<dyn LoanRepositoryPort>,
    pub auth: Arc<dyn AccessTokenVerifierPort>,
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
    pub loan_duration_days: i32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSubscriptionRequest {
    pub model_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetUnitStatusRequest {
    pub status: String,
}

pub fn build_router(state: AppState, allowed_origins: &[String]) -> Router {
    let cors = build_cors_layer(allowed_origins);

    Router::new()
        .route("/health/startup", get(health_startup))
        .route("/health/live", get(health_live))
        .route("/health/ready", get(health_ready))
        .route("/api/v1/catalogue", get(list_catalogue))
        .route(
            "/api/v1/reservations",
            post(create_reservation).get(list_reservations),
        )
        .route("/api/v1/reservations/{id}/collect", post(record_collection))
        .route("/api/v1/reservations/{id}/return", post(record_return))
        .route("/api/v1/subscriptions", post(create_subscription))
        .route("/api/v1/staff/inventory", get(list_staff_inventory))
        .route("/api/v1/staff/inventory/models", post(create_device_model))
        .route(
            "/api/v1/staff/inventory/models/{id}",
            delete(delete_device_model),
        )
        .route(
            "/api/v1/staff/inventory/models/{id}/units",
            post(add_device_unit),
        )
        .route(
            "/api/v1/staff/inventory/units/{unit_id}",
            patch(set_unit_status).delete(delete_device_unit),
        )
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
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
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

async fn list_reservations(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
        Ok(actor) => actor,
        Err(error) => return map_auth_error(error, &correlation_id).into_response(),
    };
    match state.repo.list_reservations(actor).await {
        Ok(rows) => with_correlation_header(StatusCode::OK, &correlation_id, rows).into_response(),
        Err(error) => map_app_error(error, &correlation_id).into_response(),
    }
}

async fn create_reservation(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<CreateReservationRequest>, axum::extract::rejection::JsonRejection>,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    let payload = match payload {
        Ok(Json(payload)) => payload,
        Err(_) => {
            return map_app_error(
                ApplicationError::Domain(DomainError::InvalidInput(
                    "modelId and whole-number loanDurationDays are required",
                )),
                &correlation_id,
            )
            .into_response()
        }
    };
    if let Err(err) = crate::application::validate_reserve_request(&actor, &payload.model_id) {
        return map_app_error(ApplicationError::Domain(err), &correlation_id).into_response();
    }

    match state
        .repo
        .reserve_device(
            actor,
            payload.model_id,
            payload.loan_duration_days,
            correlation_id.clone(),
        )
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
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
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
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
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
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
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

async fn list_staff_inventory(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    match state.repo.list_staff_inventory(actor).await {
        Ok(items) => with_correlation_header(
            StatusCode::OK,
            &correlation_id,
            serde_json::json!({ "items": items, "correlationId": correlation_id }),
        )
        .into_response(),
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn create_device_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload: Result<Json<CreateDeviceModelInput>, axum::extract::rejection::JsonRejection>,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    let payload = match payload {
        Ok(Json(payload)) => payload,
        Err(_) => {
            return map_app_error(
                ApplicationError::Domain(DomainError::InvalidInput(
                    "all device model fields, specifications, and initial units are required",
                )),
                &correlation_id,
            )
            .into_response()
        }
    };
    match state.repo.create_device_model(actor, payload).await {
        Ok(model) => {
            with_correlation_header(StatusCode::CREATED, &correlation_id, model).into_response()
        }
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn delete_device_model(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    match state.repo.delete_device_model(actor, id).await {
        Ok(()) => with_correlation_header(
            StatusCode::OK,
            &correlation_id,
            serde_json::json!({ "deleted": true, "correlationId": correlation_id }),
        )
        .into_response(),
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn add_device_unit(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    payload: Result<Json<NewDeviceUnitInput>, axum::extract::rejection::JsonRejection>,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    let payload = match payload {
        Ok(Json(payload)) => payload,
        Err(_) => {
            return map_app_error(
                ApplicationError::Domain(DomainError::InvalidInput(
                    "unit serialNumber and status are required",
                )),
                &correlation_id,
            )
            .into_response()
        }
    };
    match state
        .repo
        .add_device_unit(actor, id, payload, correlation_id.clone())
        .await
    {
        Ok(unit) => {
            with_correlation_header(StatusCode::CREATED, &correlation_id, unit).into_response()
        }
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn set_unit_status(
    State(state): State<AppState>,
    Path(unit_id): Path<String>,
    headers: HeaderMap,
    payload: Result<Json<SetUnitStatusRequest>, axum::extract::rejection::JsonRejection>,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    let payload = match payload {
        Ok(Json(payload)) => payload,
        Err(_) => {
            return map_app_error(
                ApplicationError::Domain(DomainError::InvalidInput(
                    "status must be AVAILABLE or MAINTENANCE",
                )),
                &correlation_id,
            )
            .into_response()
        }
    };
    match state
        .repo
        .set_unit_status(actor, unit_id, payload.status, correlation_id.clone())
        .await
    {
        Ok(unit) => with_correlation_header(StatusCode::OK, &correlation_id, unit).into_response(),
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn delete_device_unit(
    State(state): State<AppState>,
    Path(unit_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(state.auth.as_ref(), &headers).await {
        Ok(a) => a,
        Err(resp) => return map_auth_error(resp, &correlation_id).into_response(),
    };
    match state.repo.delete_device_unit(actor, unit_id).await {
        Ok(()) => with_correlation_header(
            StatusCode::OK,
            &correlation_id,
            serde_json::json!({ "deleted": true, "correlationId": correlation_id }),
        )
        .into_response(),
        Err(err) => map_app_error(err, &correlation_id).into_response(),
    }
}

async fn authenticate_request(
    verifier: &dyn AccessTokenVerifierPort,
    headers: &HeaderMap,
) -> Result<crate::domain::AuthenticatedActor, AuthVerifyError> {
    let auth_header = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());
    verifier.verify_bearer(auth_header).await
}

fn map_auth_error(err: AuthVerifyError, correlation_id: &str) -> impl IntoResponse {
    let (status, code, message) = match &err {
        AuthVerifyError::NotConfigured => (
            StatusCode::SERVICE_UNAVAILABLE,
            "AuthNotConfigured",
            "authentication is not configured on this environment",
        ),
        AuthVerifyError::MissingBearerToken => (
            StatusCode::UNAUTHORIZED,
            "Unauthorized",
            "a Bearer access token is required",
        ),
        AuthVerifyError::InvalidToken(_) => (
            StatusCode::UNAUTHORIZED,
            "InvalidToken",
            "the access token is not valid",
        ),
        AuthVerifyError::KeysUnavailable(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "AuthUnavailable",
            "token signing keys are temporarily unavailable",
        ),
    };
    // Detail stays in logs; clients get a fixed message.
    tracing::warn!(correlation_id = %correlation_id, error_code = code, error = %err, "authentication failed");
    with_correlation_header(
        status,
        correlation_id,
        ApiErrorResponse {
            error: code,
            message: message.to_string(),
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
        ApplicationError::Domain(DomainError::AlreadyExists(_)) => {
            (StatusCode::CONFLICT, "AlreadyExists")
        }
        ApplicationError::Domain(DomainError::ResourceInUse(_)) => {
            (StatusCode::CONFLICT, "ResourceInUse")
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
    use crate::test_support::FakeStore;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_and_catalogue_endpoints_respond_with_correlation_id() {
        let state = AppState {
            repo: Arc::new(FakeStore::with_units(2)),
            auth: Arc::new(crate::test_support::FakeVerifier { configured: false }),
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
                    .body(Body::from(
                        r#"{"loanDurationDays":2,"modelId":"dell-latitude-7440"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res_resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::test_support::FakeStore;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use crate::test_support::FakeVerifier;

    fn token(subject: &str, role: &str) -> String {
        format!("{subject}:{role}")
    }

    fn app_with(store: Arc<FakeStore>, configured: bool) -> Router {
        build_router(
            AppState {
                repo: store,
                auth: Arc::new(FakeVerifier { configured }),
                environment: "test".into(),
                service_version: "test".into(),
            },
            &[],
        )
    }

    #[tokio::test]
    async fn real_verifier_accepts_signed_student_token() {
        use crate::auth::test_keys::{claims, sign, KID};
        let app = build_router(
            AppState {
                repo: Arc::new(FakeStore::with_units(1)),
                auth: Arc::new(crate::infrastructure::Auth0JwtVerifier::with_test_jwks()),
                environment: "test".into(),
                service_version: "test".into(),
            },
            &[],
        );
        let bearer = sign(&claims("s1", &["student"]), KID);
        let (status, body) = request(
            &app,
            "GET",
            "/api/v1/reservations",
            Some(&bearer),
            json!(null),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!([]));
        let mut tampered = bearer.clone();
        let last = if tampered.pop() == Some('A') {
            'B'
        } else {
            'A'
        };
        tampered.push(last);
        let (status, body) = request(
            &app,
            "GET",
            "/api/v1/reservations",
            Some(&tampered),
            json!(null),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "InvalidToken");
    }

    #[tokio::test]
    async fn health_reports_verifier_configuration() {
        for configured in [true, false] {
            let app = app_with(Arc::new(FakeStore::with_units(1)), configured);
            let (status, body) = request(&app, "GET", "/health/live", None, json!(null)).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(body["auth0Configured"], json!(configured));
        }
    }

    #[tokio::test]
    async fn fake_verifier_errors_map_to_status() {
        let app = app_with(Arc::new(FakeStore::with_units(1)), true);
        let route = "/api/v1/reservations";
        let (status, body) = request(&app, "GET", route, None, json!(null)).await;
        assert_eq!(
            (status, body["error"].clone()),
            (StatusCode::UNAUTHORIZED, json!("Unauthorized"))
        );
        let (status, body) = request(&app, "GET", route, Some("garbage"), json!(null)).await;
        assert_eq!(
            (status, body["error"].clone()),
            (StatusCode::UNAUTHORIZED, json!("InvalidToken"))
        );
        let (status, body) = request(&app, "GET", route, Some("keys-down"), json!(null)).await;
        assert_eq!(
            (status, body["error"].clone()),
            (StatusCode::SERVICE_UNAVAILABLE, json!("AuthUnavailable"))
        );
        let app = app_with(Arc::new(FakeStore::with_units(1)), false);
        let (status, body) = request(
            &app,
            "GET",
            route,
            Some(&token("a", "student")),
            json!(null),
        )
        .await;
        assert_eq!(
            (status, body["error"].clone()),
            (StatusCode::SERVICE_UNAVAILABLE, json!("AuthNotConfigured"))
        );
    }
    async fn request(
        app: &Router,
        method: &str,
        path: &str,
        bearer: Option<&str>,
        body: Value,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("x-correlation-id", "contract-test");
        if let Some(token) = bearer {
            request = request.header("authorization", format!("Bearer {token}"));
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()["x-correlation-id"], "contract-test");
        let status = response.status();
        let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }
    #[tokio::test]
    async fn invalid_or_missing_periods_return_400_without_writes() {
        let store = Arc::new(FakeStore::with_units(1));
        let app = app_with(store.clone(), true);
        let owner = token("owner", "student");
        for body in [
            json!({"modelId":"model"}),
            json!({"modelId":"model","loanDurationDays":null}),
            json!({"modelId":"model","loanDurationDays":1.5}),
            json!({"modelId":"model","loanDurationDays":"7"}),
            json!({"modelId":"model","loanDurationDays":0}),
            json!({"modelId":"model","loanDurationDays":15}),
        ] {
            let (status, error) =
                request(&app, "POST", "/api/v1/reservations", Some(&owner), body).await;
            assert_eq!(status, StatusCode::BAD_REQUEST);
            assert_eq!(error["error"], "InvalidInput");
        }
        let data = store.data.lock().await;
        assert_eq!(data.available, 1);
        assert!(data.reservations.is_empty());
        assert!(data.outbox.is_empty());
    }

    #[tokio::test]
    async fn authenticated_routes_due_date_errors_and_owner_reads() {
        let store = Arc::new(FakeStore::with_units(1));
        let app = app_with(store.clone(), true);
        let owner = token("owner", "student");
        let staff = token("staff", "staff");
        let other = token("other", "student");
        let route = "/api/v1/reservations";
        assert_eq!(
            request(
                &app,
                "POST",
                route,
                None,
                json!({"loanDurationDays":2,"modelId":"model"})
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
        let (status, body) = request(
            &app,
            "POST",
            route,
            Some(&owner),
            json!({"loanDurationDays":2,"modelId":""}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "InvalidInput");
        let (status, body) = request(
            &app,
            "POST",
            route,
            Some(&owner),
            json!({"loanDurationDays":2,"modelId":"missing"}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "NotFound");
        let (status, reservation) = request(
            &app,
            "POST",
            route,
            Some(&owner),
            json!({"loanDurationDays":2,"modelId":"model"}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let reserved: chrono::DateTime<chrono::Utc> =
            reservation["reservedAt"].as_str().unwrap().parse().unwrap();
        let due: chrono::DateTime<chrono::Utc> =
            reservation["dueAt"].as_str().unwrap().parse().unwrap();
        assert_eq!(due - reserved, chrono::Duration::days(2));
        let id = reservation["reservationId"].as_str().unwrap();
        let collect = format!("{route}/{id}/collect");
        let ret = format!("{route}/{id}/return");
        let (status, body) = request(&app, "POST", &collect, Some(&owner), json!({})).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"], "ForbiddenRole");
        let (status, body) = request(&app, "POST", &ret, Some(&staff), json!({})).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "InvalidTransition");
        let (status, body) = request(
            &app,
            "POST",
            route,
            Some(&other),
            json!({"loanDurationDays":2,"modelId":"model"}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "NoAvailableUnits");
        let (status, sub) = request(
            &app,
            "POST",
            "/api/v1/subscriptions",
            Some(&other),
            json!({"loanDurationDays":2,"modelId":"model"}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let (_, duplicate) = request(
            &app,
            "POST",
            "/api/v1/subscriptions",
            Some(&other),
            json!({"loanDurationDays":2,"modelId":"model"}),
        )
        .await;
        assert_eq!(sub["subscriptionId"], duplicate["subscriptionId"]);
        assert_eq!(
            request(&app, "GET", route, Some(&other), json!(null))
                .await
                .1,
            json!([])
        );
        assert_eq!(
            request(&app, "GET", route, Some(&owner), json!(null))
                .await
                .1
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            request(&app, "GET", route, Some(&staff), json!(null))
                .await
                .1
                .as_array()
                .unwrap()
                .len(),
            1
        );
        *store.clock.lock().unwrap() += chrono::Duration::hours(6);
        let (status, collected) = request(&app, "POST", &collect, Some(&staff), json!({})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(collected["dueAt"], reservation["dueAt"]);
        let (status, returned) = request(&app, "POST", &ret, Some(&staff), json!({})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(returned["outboxEventIds"].as_array().unwrap().len(), 2);
        assert_eq!(
            request(&app, "POST", &ret, Some(&staff), json!({})).await.0,
            StatusCode::CONFLICT
        );
    }

    #[tokio::test]
    async fn staff_inventory_crud_maintenance_and_guards() {
        let store = Arc::new(FakeStore::with_units(1));
        let app = app_with(store.clone(), true);
        let student = token("stud", "student");
        let staff = token("staff", "staff");

        // Student cannot read or mutate staff inventory
        let (status, body) = request(
            &app,
            "GET",
            "/api/v1/staff/inventory",
            Some(&student),
            json!(null),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["error"], "ForbiddenRole");

        let new_model = json!({
            "id": "sony-fx3",
            "name": "Sony FX3 Cinema Kit",
            "category": "Camera",
            "description": "Full-frame cinema camera.",
            "loanDurationDays": 7,
            "imageUrl": "data:image/webp;base64,UklGRh4AAABXRUJQVlA4",
            "imageAlt": "Sony FX3 Cinema Kit",
            "specifications": [{"label": "Sensor", "value": "Full-frame"}],
            "units": [{"serialNumber": "FX3-001", "status": "MAINTENANCE"}]
        });
        let (status, _) = request(
            &app,
            "POST",
            "/api/v1/staff/inventory/models",
            Some(&student),
            new_model.clone(),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);

        // Missing mandatory imageUrl fails validation
        let (status, body) = request(
            &app,
            "POST",
            "/api/v1/staff/inventory/models",
            Some(&staff),
            json!({
                "id": "bad-model",
                "name": "Bad",
                "category": "Camera",
                "description": "Missing image",
                "loanDurationDays": 7,
                "imageUrl": "",
                "imageAlt": "Bad",
                "specifications": [{"label": "A", "value": "B"}],
                "units": [{"serialNumber": "SN-1", "status": "AVAILABLE"}]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "InvalidInput");

        // Staff creates model starting in MAINTENANCE (0 available)
        let (status, created) = request(
            &app,
            "POST",
            "/api/v1/staff/inventory/models",
            Some(&staff),
            new_model,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created["totalUnits"], 1);
        assert_eq!(created["availableUnits"], 0);
        let unit_id = created["units"][0]["unitId"].as_str().unwrap().to_string();

        // Student subscribes while 0 available
        let (status, _) = request(
            &app,
            "POST",
            "/api/v1/subscriptions",
            Some(&student),
            json!({"modelId": "sony-fx3"}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        // Moving unit from MAINTENANCE -> AVAILABLE emits availability notification outbox event
        let (status, updated) = request(
            &app,
            "PATCH",
            &format!("/api/v1/staff/inventory/units/{unit_id}"),
            Some(&staff),
            json!({"status": "AVAILABLE"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(updated["status"], "AVAILABLE");
        assert_eq!(store.data.lock().await.outbox.len(), 1);

        // Add second unit, reserve first, ensure maintenance & delete fail for reserved unit
        let (status, added_unit) = request(
            &app,
            "POST",
            "/api/v1/staff/inventory/models/sony-fx3/units",
            Some(&staff),
            json!({"serialNumber": "FX3-002", "status": "AVAILABLE"}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let second_unit = added_unit["unitId"].as_str().unwrap().to_string();

        let (status, _) = request(
            &app,
            "POST",
            "/api/v1/reservations",
            Some(&student),
            json!({"modelId": "sony-fx3", "loanDurationDays": 2}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let (status, body) = request(
            &app,
            "PATCH",
            &format!("/api/v1/staff/inventory/units/{unit_id}"),
            Some(&staff),
            json!({"status": "MAINTENANCE"}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "InvalidTransition");

        let (status, body) = request(
            &app,
            "DELETE",
            &format!("/api/v1/staff/inventory/units/{unit_id}"),
            Some(&staff),
            json!(null),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "ResourceInUse");

        // Unreserved second unit can be moved to maintenance and removed
        let (status, _) = request(
            &app,
            "PATCH",
            &format!("/api/v1/staff/inventory/units/{second_unit}"),
            Some(&staff),
            json!({"status": "MAINTENANCE"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        let (status, _) = request(
            &app,
            "DELETE",
            &format!("/api/v1/staff/inventory/units/{second_unit}"),
            Some(&staff),
            json!(null),
        )
        .await;
        assert_eq!(status, StatusCode::OK);

        // Fresh unloaned model can be created and deleted cleanly
        let (status, _) = request(
            &app,
            "POST",
            "/api/v1/staff/inventory/models",
            Some(&staff),
            json!({
                "id": "temp-mic",
                "name": "Temp Mic",
                "category": "Audio",
                "description": "Disposable test mic.",
                "loanDurationDays": 3,
                "imageUrl": "/catalogue/blue-yeti.webp",
                "imageAlt": "Temp Mic",
                "specifications": [{"label": "Type", "value": "USB"}],
                "units": [{"serialNumber": "TEMP-1", "status": "AVAILABLE"}]
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let (status, _) = request(
            &app,
            "DELETE",
            "/api/v1/staff/inventory/models/temp-mic",
            Some(&staff),
            json!(null),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }
}
