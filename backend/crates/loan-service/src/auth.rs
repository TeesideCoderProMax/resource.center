//! Auth0 access-token verification (infrastructure adapter).
//!
//! Identity rules:
//! - `sub` is the actor identity.
//! - Roles come only from the configured namespaced claim (an Auth0 Action sets it).
//! - The notification address comes only from the configured namespaced email claim.
//!   A missing or blank email claim fails closed; there is no invented fallback address.
//!
//! JWKS rules: cache for `JWKS_TTL`; an unknown `kid` forces one refresh, rate limited by
//! `JWKS_MIN_REFRESH`; when a refresh fails, a cached set is still used. If no usable key
//! set exists, the request fails with `KeysUnavailable` (503), never as authenticated.

use std::sync::Arc;
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::domain::{AuthenticatedActor, Role};

const JWKS_TTL: Duration = Duration::from_secs(600);
const JWKS_MIN_REFRESH: Duration = Duration::from_secs(30);
const DEFAULT_ROLES_CLAIM: &str = "https://resource-center.campus/roles";
const DEFAULT_EMAIL_CLAIM: &str = "https://resource-center.campus/email";

#[derive(Default)]
struct JwksCache {
    set: Option<JwkSet>,
    fetched_at: Option<Instant>,
}

#[derive(Clone)]
pub struct Auth0JwtVerifier {
    issuer: Option<String>,
    audience: Option<String>,
    jwks_url: String,
    role_claim: String,
    email_claim: String,
    http: reqwest::Client,
    cache: Arc<Mutex<JwksCache>>,
}

#[derive(Debug, Deserialize)]
struct Claims {
    sub: String,
    #[serde(flatten)]
    extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
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

fn non_blank_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

impl Auth0JwtVerifier {
    pub fn new(
        issuer: Option<String>,
        audience: Option<String>,
        role_claim: String,
        email_claim: String,
    ) -> Self {
        let jwks_url = issuer
            .as_deref()
            .map(|i| format!("{}/.well-known/jwks.json", i.trim_end_matches('/')))
            .unwrap_or_default();
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client");
        Self {
            issuer,
            audience,
            jwks_url,
            role_claim,
            email_claim,
            http,
            cache: Arc::new(Mutex::new(JwksCache::default())),
        }
    }

    pub fn from_env() -> Self {
        Self::new(
            // Terraform sets AUTH0_DOMAIN; Auth0 issuers are `https://<domain>/`.
            non_blank_env("AUTH0_ISSUER").or_else(|| {
                non_blank_env("AUTH0_DOMAIN").map(|d| {
                    format!(
                        "https://{}/",
                        d.trim_start_matches("https://").trim_end_matches('/')
                    )
                })
            }),
            non_blank_env("AUTH0_AUDIENCE"),
            non_blank_env("AUTH0_ROLES_CLAIM").unwrap_or_else(|| DEFAULT_ROLES_CLAIM.into()),
            non_blank_env("AUTH0_EMAIL_CLAIM").unwrap_or_else(|| DEFAULT_EMAIL_CLAIM.into()),
        )
    }

    #[cfg(test)]
    pub fn with_jwks_url(mut self, url: String) -> Self {
        self.jwks_url = url;
        self
    }

    #[cfg(test)]
    pub fn with_test_jwks() -> Self {
        let v = Self::new(
            Some(crate::auth::test_keys::ISSUER.into()),
            Some(crate::auth::test_keys::AUDIENCE.into()),
            "roles".into(),
            "email".into(),
        );
        v.cache.try_lock().unwrap().set =
            Some(serde_json::from_str(crate::auth::test_keys::JWKS).unwrap());
        v.cache.try_lock().unwrap().fetched_at = Some(Instant::now());
        v
    }

    pub fn is_configured(&self) -> bool {
        self.issuer.is_some() && self.audience.is_some()
    }

    pub async fn verify_bearer(
        &self,
        auth_header: Option<&str>,
    ) -> Result<AuthenticatedActor, AuthVerifyError> {
        let (Some(issuer), Some(audience)) = (&self.issuer, &self.audience) else {
            return Err(AuthVerifyError::NotConfigured);
        };
        let token = auth_header
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or(AuthVerifyError::MissingBearerToken)?;

        let header = decode_header(token)
            .map_err(|e| AuthVerifyError::InvalidToken(format!("jwt header: {e}")))?;
        if header.alg != Algorithm::RS256 {
            return Err(AuthVerifyError::InvalidToken("unsupported alg".into()));
        }
        let kid = header
            .kid
            .ok_or_else(|| AuthVerifyError::InvalidToken("missing kid".into()))?;
        let key = self.decoding_key(&kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[issuer.as_str()]);
        validation.set_audience(&[audience.as_str()]);
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
        validation.leeway = 30;

        let claims = decode::<Claims>(token, &key, &validation)
            .map_err(|e| AuthVerifyError::InvalidToken(format!("claims: {e}")))?
            .claims;
        if claims.sub.trim().is_empty() {
            return Err(AuthVerifyError::InvalidToken("blank sub".into()));
        }

        let mut roles = Vec::new();
        if let Some(serde_json::Value::Array(items)) = claims.extra.get(&self.role_claim) {
            for role in items.iter().filter_map(|v| v.as_str()) {
                let parsed = match role.to_ascii_lowercase().as_str() {
                    "student" => Role::Student,
                    "staff" => Role::Staff,
                    _ => continue,
                };
                if !roles.contains(&parsed) {
                    roles.push(parsed);
                }
            }
        }

        let email = claims
            .extra
            .get(&self.email_claim)
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|e| !e.is_empty() && e.contains('@'))
            .ok_or_else(|| AuthVerifyError::InvalidToken("missing trusted email claim".into()))?
            .to_string();

        Ok(AuthenticatedActor {
            subject: claims.sub,
            email,
            roles,
        })
    }

    async fn decoding_key(&self, kid: &str) -> Result<DecodingKey, AuthVerifyError> {
        let mut cache = self.cache.lock().await;
        let fresh = cache.fetched_at.is_some_and(|t| t.elapsed() < JWKS_TTL);
        let cached_key = cache.set.as_ref().and_then(|s| s.find(kid)).cloned();

        if let (true, Some(jwk)) = (fresh, &cached_key) {
            return DecodingKey::from_jwk(jwk)
                .map_err(|e| AuthVerifyError::InvalidToken(format!("jwk: {e}")));
        }
        // Stale cache or unknown kid: refresh, but not more than once per JWKS_MIN_REFRESH.
        let may_refresh = cache
            .fetched_at
            .is_none_or(|t| t.elapsed() >= JWKS_MIN_REFRESH);
        if may_refresh {
            match self.fetch_jwks().await {
                Ok(set) => {
                    cache.set = Some(set);
                    cache.fetched_at = Some(Instant::now());
                }
                Err(e) if cached_key.is_none() && cache.set.is_none() => return Err(e),
                Err(e) => tracing::warn!(error = %e, "JWKS refresh failed; using cached keys"),
            }
        }
        match cache.set.as_ref().and_then(|s| s.find(kid)) {
            Some(jwk) => DecodingKey::from_jwk(jwk)
                .map_err(|e| AuthVerifyError::InvalidToken(format!("jwk: {e}"))),
            None => Err(AuthVerifyError::InvalidToken(format!("unknown kid {kid}"))),
        }
    }

    async fn fetch_jwks(&self) -> Result<JwkSet, AuthVerifyError> {
        let resp = self
            .http
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| AuthVerifyError::KeysUnavailable(format!("fetch: {e}")))?;
        if !resp.status().is_success() {
            return Err(AuthVerifyError::KeysUnavailable(format!(
                "status {}",
                resp.status()
            )));
        }
        resp.json()
            .await
            .map_err(|e| AuthVerifyError::KeysUnavailable(format!("parse: {e}")))
    }
}

#[cfg(test)]
pub mod test_keys {
    use jsonwebtoken::{encode, EncodingKey, Header};
    use serde_json::{json, Value};

    pub const ISSUER: &str = "https://issuer.example.invalid/";
    pub const AUDIENCE: &str = "loan-test";
    pub const KID: &str = "phase1-test";
    pub const JWKS: &str = include_str!("../tests/fixtures/jwks.json");
    const PEM: &[u8] = include_bytes!("../tests/fixtures/test-only-rsa.pem");

    pub fn claims(subject: &str, roles: &[&str]) -> Value {
        json!({
            "sub": subject,
            "email": format!("{subject}@example.invalid"),
            "roles": roles,
            "iss": ISSUER,
            "aud": AUDIENCE,
            "exp": chrono::Utc::now().timestamp() + 600,
        })
    }

    pub fn sign(claims: &Value, kid: &str) -> String {
        let mut header = Header::new(jsonwebtoken::Algorithm::RS256);
        header.kid = Some(kid.into());
        encode(&header, claims, &EncodingKey::from_rsa_pem(PEM).unwrap()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_keys::*;
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn bearer(t: &str) -> String {
        format!("Bearer {t}")
    }

    async fn verify(
        v: &Auth0JwtVerifier,
        claims: serde_json::Value,
    ) -> Result<AuthenticatedActor, AuthVerifyError> {
        v.verify_bearer(Some(&bearer(&sign(&claims, KID)))).await
    }

    /// Serves the fixture JWKS (or 500 when `fail` is set) and counts requests.
    async fn jwks_server(fail: bool) -> (String, Arc<AtomicUsize>) {
        let hits = Arc::new(AtomicUsize::new(0));
        let h = hits.clone();
        let app = axum::Router::new().route(
            "/jwks",
            axum::routing::get(move || {
                let h = h.clone();
                async move {
                    h.fetch_add(1, Ordering::SeqCst);
                    if fail {
                        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, String::new())
                    } else {
                        (axum::http::StatusCode::OK, JWKS.to_string())
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}/jwks"), hits)
    }

    fn remote_verifier(url: String) -> Auth0JwtVerifier {
        Auth0JwtVerifier::new(
            Some(ISSUER.into()),
            Some(AUDIENCE.into()),
            "roles".into(),
            "email".into(),
        )
        .with_jwks_url(url)
    }

    #[tokio::test]
    async fn rejects_missing_unconfigured_and_tampered_tokens() {
        let v = Auth0JwtVerifier::with_test_jwks();
        assert!(matches!(
            v.verify_bearer(None).await,
            Err(AuthVerifyError::MissingBearerToken)
        ));
        assert!(matches!(
            v.verify_bearer(Some("Basic x")).await,
            Err(AuthVerifyError::MissingBearerToken)
        ));
        assert!(matches!(
            v.verify_bearer(Some("Bearer ")).await,
            Err(AuthVerifyError::MissingBearerToken)
        ));

        let unconfigured = Auth0JwtVerifier::new(None, None, "roles".into(), "email".into());
        let token = sign(&claims("auth0|a", &["student"]), KID);
        assert!(matches!(
            unconfigured.verify_bearer(Some(&bearer(&token))).await,
            Err(AuthVerifyError::NotConfigured)
        ));

        // Bad signature: change one payload character.
        let mut parts: Vec<String> = token.split('.').map(String::from).collect();
        parts[1] = sign(&claims("auth0|attacker", &["staff"]), KID)
            .split('.')
            .nth(1)
            .unwrap()
            .into();
        parts[2] = parts[2].chars().rev().collect();
        let forged = parts.join(".");
        assert!(matches!(
            v.verify_bearer(Some(&bearer(&forged))).await,
            Err(AuthVerifyError::InvalidToken(_))
        ));
    }

    #[tokio::test]
    async fn validates_issuer_audience_expiry_and_trusted_claims() {
        let v = Auth0JwtVerifier::with_test_jwks();
        let mut c = claims("auth0|s", &["student"]);
        c["iss"] = json!("https://evil.example.invalid/");
        assert!(matches!(
            verify(&v, c).await,
            Err(AuthVerifyError::InvalidToken(_))
        ));

        let mut c = claims("auth0|s", &["student"]);
        c["aud"] = json!("other-api");
        assert!(matches!(
            verify(&v, c).await,
            Err(AuthVerifyError::InvalidToken(_))
        ));

        let mut c = claims("auth0|s", &["student"]);
        c["exp"] = json!(chrono::Utc::now().timestamp() - 120);
        assert!(matches!(
            verify(&v, c).await,
            Err(AuthVerifyError::InvalidToken(_))
        ));

        let mut c = claims("auth0|s", &["student"]);
        c.as_object_mut().unwrap().remove("email");
        assert!(matches!(
            verify(&v, c).await,
            Err(AuthVerifyError::InvalidToken(_))
        ));

        // Roles only from the configured claim; untrusted claims and unknown roles are ignored.
        let mut c = claims("auth0|s", &["Staff", "admin", "staff"]);
        c["permissions"] = json!(["student"]);
        let actor = verify(&v, c).await.unwrap();
        assert_eq!(actor.roles, vec![Role::Staff]);
        assert_eq!(actor.subject, "auth0|s");
        assert_eq!(actor.email, "auth0|s@example.invalid");

        let mut c = claims("auth0|n", &[]);
        c.as_object_mut().unwrap().remove("roles");
        c["permissions"] = json!(["staff"]);
        assert!(verify(&v, c).await.unwrap().roles.is_empty());
    }

    #[tokio::test]
    async fn jwks_is_fetched_cached_and_refreshed_for_unknown_kid() {
        let (url, hits) = jwks_server(false).await;
        let v = remote_verifier(url);
        verify(&v, claims("auth0|a", &["student"])).await.unwrap();
        verify(&v, claims("auth0|a", &["student"])).await.unwrap();
        assert_eq!(hits.load(Ordering::SeqCst), 1, "cached JWKS reused");

        // Unknown kid within the refresh limit: no extra fetch, rejected as invalid.
        let other = sign(&claims("auth0|a", &["student"]), "rotated-kid");
        assert!(matches!(
            v.verify_bearer(Some(&bearer(&other))).await,
            Err(AuthVerifyError::InvalidToken(_))
        ));
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // After the limit, an unknown kid forces one refresh.
        v.cache.lock().await.fetched_at = Some(Instant::now() - JWKS_MIN_REFRESH);
        assert!(v.verify_bearer(Some(&bearer(&other))).await.is_err());
        assert_eq!(hits.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn jwks_failure_fails_closed_or_uses_cached_keys() {
        let (url, _) = jwks_server(true).await;
        let v = remote_verifier(url.clone());
        assert!(matches!(
            verify(&v, claims("auth0|a", &["student"])).await,
            Err(AuthVerifyError::KeysUnavailable(_))
        ));

        let v = Auth0JwtVerifier::with_test_jwks().with_jwks_url(url);
        v.cache.lock().await.fetched_at = Some(Instant::now() - JWKS_TTL);
        verify(&v, claims("auth0|a", &["student"])).await.unwrap();
    }
}
