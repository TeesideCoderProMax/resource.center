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
        .route(
            "/api/v1/reservations",
            post(create_reservation).get(list_reservations),
        )
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

async fn list_reservations(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let correlation_id = extract_correlation_id(&headers);
    let actor = match authenticate_request(&state.auth, &headers).await {
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
    use crate::test_support::FakeStore;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_and_catalogue_endpoints_respond_with_correlation_id() {
        let state = AppState {
            repo: Arc::new(FakeStore::with_units(2)),
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
}

#[cfg(test)]
mod contract_tests {
    use super::*;
    use crate::test_support::FakeStore;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
    use serde_json::{json, Value};
    use tower::ServiceExt;

    fn token(subject: &str, role: &str) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some("phase1-test".into());
        encode(&header, &json!({"sub":subject,"email":format!("{subject}@example.invalid"),"roles":[role],"iss":"https://issuer.example.invalid/","aud":"loan-test","exp":chrono::Utc::now().timestamp()+600}), &EncodingKey::from_rsa_pem(include_bytes!("../tests/fixtures/test-only-rsa.pem")).unwrap()).unwrap()
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
    async fn authenticated_routes_due_date_errors_and_owner_reads() {
        let store = Arc::new(FakeStore::with_units(1));
        let app = build_router(
            AppState {
                repo: store.clone(),
                auth: Auth0JwtVerifier::with_test_jwks(),
                environment: "test".into(),
                service_version: "test".into(),
            },
            &[],
        );
        let owner = token("owner", "student");
        let staff = token("staff", "staff");
        let other = token("other", "student");
        let route = "/api/v1/reservations";
        assert_eq!(
            request(&app, "POST", route, None, json!({"modelId":"model"}))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let (status, body) =
            request(&app, "POST", route, Some(&owner), json!({"modelId":""})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "InvalidInput");
        let (status, body) = request(
            &app,
            "POST",
            route,
            Some(&owner),
            json!({"modelId":"missing"}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "NotFound");
        let (status, reservation) = request(
            &app,
            "POST",
            route,
            Some(&owner),
            json!({"modelId":"model"}),
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
            json!({"modelId":"model"}),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "NoAvailableUnits");
        let (status, sub) = request(
            &app,
            "POST",
            "/api/v1/subscriptions",
            Some(&other),
            json!({"modelId":"model"}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let (_, duplicate) = request(
            &app,
            "POST",
            "/api/v1/subscriptions",
            Some(&other),
            json!({"modelId":"model"}),
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
}
