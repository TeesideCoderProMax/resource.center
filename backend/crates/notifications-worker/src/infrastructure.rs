use crate::application::{
    CreateJobOutcome, EmailDeliveryPort, EmailDeliveryResult, JobStoreError,
    NotificationJobStorePort, ReplaceJobOutcome, VersionedJobDocument,
};
use crate::domain::{DeliveryError, NotificationJobDocument};
use async_trait::async_trait;
use chrono::Utc;
use native_tls::TlsConnector;
use postgres_native_tls::MakeTlsConnector;
use rc_shared::{redact_email, ManagedIdentityTokenProvider, NotificationRequestV1};
use reqwest::StatusCode;
use serde::Deserialize;
use std::collections::HashSet;
use std::sync::Arc;
use tokio_postgres::Config as PgConfig;
use tracing::info;

const COSMOS_SCOPE: &str = "https://cosmos.azure.com";
const SERVICEBUS_SCOPE: &str = "https://servicebus.azure.net";
const KEYVAULT_SCOPE: &str = "https://vault.azure.net";
const POSTGRES_SCOPE: &str = "https://ossrdbms-aad.database.windows.net";
const COSMOS_API_VERSION: &str = "2018-12-31";

pub struct CosmosRestJobStore {
    endpoint: String,
    database: String,
    container: String,
    token_provider: Arc<ManagedIdentityTokenProvider>,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct CosmosSystemFields {
    #[serde(rename = "_etag")]
    etag: String,
}

impl CosmosRestJobStore {
    pub fn new(
        endpoint: String,
        database: String,
        container: String,
        token_provider: Arc<ManagedIdentityTokenProvider>,
    ) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            database,
            container,
            token_provider,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
        }
    }

    async fn auth_header(&self) -> Result<String, JobStoreError> {
        let token = self
            .token_provider
            .get_token(COSMOS_SCOPE)
            .await
            .map_err(|e| JobStoreError::Storage(format!("cosmos token acquisition failed: {e}")))?;
        Ok(format!(
            "type%3daad%26ver%3d1.0%26sig%3d{}",
            urlencoding::encode(&token)
        ))
    }

    fn docs_collection_url(&self) -> String {
        format!(
            "{}/dbs/{}/colls/{}/docs",
            self.endpoint, self.database, self.container
        )
    }

    fn doc_item_url(&self, id: &str) -> String {
        format!(
            "{}/dbs/{}/colls/{}/docs/{}",
            self.endpoint,
            self.database,
            self.container,
            urlencoding::encode(id)
        )
    }

    fn partition_key_header(event_id: &str) -> Result<String, JobStoreError> {
        serde_json::to_string(&[event_id])
            .map_err(|e| JobStoreError::Storage(format!("partition key serialize failed: {e}")))
    }

    fn rfc7231_now() -> String {
        Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string()
    }

    fn parse_versioned(
        status: StatusCode,
        headers: &reqwest::header::HeaderMap,
        body: &str,
    ) -> Result<VersionedJobDocument, JobStoreError> {
        let document: NotificationJobDocument = serde_json::from_str(body).map_err(|e| {
            JobStoreError::Storage(format!(
                "failed to deserialize cosmos job document (status {status}): {e}"
            ))
        })?;
        let header_etag = headers
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let etag = match header_etag {
            Some(e) => e,
            None => {
                let sys: CosmosSystemFields = serde_json::from_str(body).map_err(|e| {
                    JobStoreError::Storage(format!("missing etag in cosmos response: {e}"))
                })?;
                sys.etag
            }
        };
        Ok(VersionedJobDocument { document, etag })
    }
}

#[async_trait]
impl NotificationJobStorePort for CosmosRestJobStore {
    async fn create_job(
        &self,
        doc: &NotificationJobDocument,
    ) -> Result<CreateJobOutcome, JobStoreError> {
        let auth = self.auth_header().await?;
        let pk = Self::partition_key_header(&doc.event_id)?;
        let resp = self
            .http
            .post(self.docs_collection_url())
            .header("authorization", auth)
            .header("x-ms-version", COSMOS_API_VERSION)
            .header("x-ms-date", Self::rfc7231_now())
            .header("x-ms-documentdb-partitionkey", pk)
            .header("Content-Type", "application/json")
            .json(doc)
            .send()
            .await
            .map_err(|e| JobStoreError::Storage(format!("cosmos POST doc failed: {e}")))?;

        let status = resp.status();
        if status == StatusCode::CONFLICT {
            return Ok(CreateJobOutcome::AlreadyExists);
        }
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(JobStoreError::Storage(format!(
                "cosmos create_job failed with HTTP {status}"
            )));
        }

        let versioned = Self::parse_versioned(status, &headers, &body)?;
        Ok(CreateJobOutcome::Created(versioned))
    }

    async fn get_job(&self, event_id: &str) -> Result<Option<VersionedJobDocument>, JobStoreError> {
        let auth = self.auth_header().await?;
        let pk = Self::partition_key_header(event_id)?;
        let resp = self
            .http
            .get(self.doc_item_url(event_id))
            .header("authorization", auth)
            .header("x-ms-version", COSMOS_API_VERSION)
            .header("x-ms-date", Self::rfc7231_now())
            .header("x-ms-documentdb-partitionkey", pk)
            .send()
            .await
            .map_err(|e| JobStoreError::Storage(format!("cosmos GET doc failed: {e}")))?;

        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(JobStoreError::Storage(format!(
                "cosmos get_job failed with HTTP {status}"
            )));
        }

        let versioned = Self::parse_versioned(status, &headers, &body)?;
        Ok(Some(versioned))
    }

    async fn replace_job(
        &self,
        doc: &NotificationJobDocument,
        if_match_etag: &str,
    ) -> Result<ReplaceJobOutcome, JobStoreError> {
        let auth = self.auth_header().await?;
        let pk = Self::partition_key_header(&doc.event_id)?;
        let resp = self
            .http
            .put(self.doc_item_url(&doc.event_id))
            .header("authorization", auth)
            .header("x-ms-version", COSMOS_API_VERSION)
            .header("x-ms-date", Self::rfc7231_now())
            .header("x-ms-documentdb-partitionkey", pk)
            .header("If-Match", if_match_etag)
            .header("Content-Type", "application/json")
            .json(doc)
            .send()
            .await
            .map_err(|e| JobStoreError::Storage(format!("cosmos PUT doc failed: {e}")))?;

        let status = resp.status();
        if status == StatusCode::PRECONDITION_FAILED {
            return Ok(ReplaceJobOutcome::PreconditionFailed);
        }
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(JobStoreError::Storage(format!(
                "cosmos replace_job failed with HTTP {status}"
            )));
        }

        let versioned = Self::parse_versioned(status, &headers, &body)?;
        Ok(ReplaceJobOutcome::Replaced(versioned))
    }

    async fn delete_job(&self, event_id: &str) -> Result<(), JobStoreError> {
        let auth = self.auth_header().await?;
        let pk = Self::partition_key_header(event_id)?;
        let resp = self
            .http
            .delete(self.doc_item_url(event_id))
            .header("authorization", auth)
            .header("x-ms-version", COSMOS_API_VERSION)
            .header("x-ms-date", Self::rfc7231_now())
            .header("x-ms-documentdb-partitionkey", pk)
            .send()
            .await
            .map_err(|e| JobStoreError::Storage(format!("cosmos DELETE doc failed: {e}")))?;

        let status = resp.status();
        if status == StatusCode::NOT_FOUND || status.is_success() {
            return Ok(());
        }
        Err(JobStoreError::Storage(format!(
            "cosmos delete_job failed with HTTP {status}"
        )))
    }
}

#[derive(Debug, Clone)]
pub struct PeekLockedMessage {
    pub body: String,
    pub lock_location: String,
    pub message_id: Option<String>,
    pub delivery_count: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BrokerPropertiesHeader {
    message_id: Option<String>,
    delivery_count: Option<u32>,
}

pub struct ServiceBusRestConsumer {
    namespace_fqdn: String,
    queue_name: String,
    token_provider: Arc<ManagedIdentityTokenProvider>,
    http: reqwest::Client,
}

impl ServiceBusRestConsumer {
    pub fn new(
        namespace_fqdn: String,
        queue_name: String,
        token_provider: Arc<ManagedIdentityTokenProvider>,
    ) -> Self {
        Self {
            namespace_fqdn,
            queue_name,
            token_provider,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
        }
    }

    async fn bearer_token(&self) -> Result<String, String> {
        self.token_provider
            .get_token(SERVICEBUS_SCOPE)
            .await
            .map_err(|e| format!("service bus token error: {e}"))
    }

    /// Receive and lock the next available message from the queue (`POST .../messages/head?timeout={wait_seconds}`).
    /// Returns `Ok(None)` when the queue is empty (`204 No Content`).
    pub async fn receive_peek_lock(
        &self,
        wait_seconds: u32,
    ) -> Result<Option<PeekLockedMessage>, String> {
        let token = self.bearer_token().await?;
        let url = format!(
            "https://{}/{}/messages/head?timeout={}",
            self.namespace_fqdn, self.queue_name, wait_seconds
        );
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&token)
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| format!("service bus receive request failed: {e}"))?;

        let status = resp.status();
        if status == StatusCode::NO_CONTENT {
            return Ok(None);
        }
        if status != StatusCode::CREATED && !status.is_success() {
            return Err(format!("service bus receive returned HTTP {status}"));
        }

        let lock_location = resp
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| "missing Location lock header in Service Bus response".to_string())?
            .to_string();

        let broker_props = resp
            .headers()
            .get("BrokerProperties")
            .and_then(|v| v.to_str().ok())
            .and_then(|json| serde_json::from_str::<BrokerPropertiesHeader>(json).ok());

        let body = resp
            .text()
            .await
            .map_err(|e| format!("failed reading Service Bus message body: {e}"))?;

        Ok(Some(PeekLockedMessage {
            body,
            lock_location,
            message_id: broker_props.as_ref().and_then(|p| p.message_id.clone()),
            delivery_count: broker_props.and_then(|p| p.delivery_count),
        }))
    }

    pub async fn renew_lock(&self, lock_location: &str) -> Result<(), String> {
        let token = self.bearer_token().await?;
        let resp = self
            .http
            .post(lock_location)
            .bearer_auth(&token)
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| format!("service bus lock renew failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "service bus lock renew returned HTTP {}",
                resp.status()
            ));
        }
        Ok(())
    }

    pub async fn complete_message(&self, lock_location: &str) -> Result<(), String> {
        let token = self.bearer_token().await?;
        let resp = self
            .http
            .delete(lock_location)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("service bus message complete failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            return Err(format!("service bus complete returned HTTP {status}"));
        }
        Ok(())
    }

    pub async fn abandon_message(&self, lock_location: &str) -> Result<(), String> {
        let token = self.bearer_token().await?;
        let resp = self
            .http
            .put(lock_location)
            .bearer_auth(&token)
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| format!("service bus message abandon failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            return Err(format!("service bus abandon returned HTTP {status}"));
        }
        Ok(())
    }
}

pub struct KeyVaultSecretClient {
    vault_uri: String,
    token_provider: Arc<ManagedIdentityTokenProvider>,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct KeyVaultSecretBundle {
    value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretLookupOutcome {
    Found(String),
    NotFound,
}

impl KeyVaultSecretClient {
    pub fn new(vault_uri: String, token_provider: Arc<ManagedIdentityTokenProvider>) -> Self {
        Self {
            vault_uri: vault_uri.trim_end_matches('/').to_string(),
            token_provider,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
        }
    }

    pub async fn get_secret(&self, secret_name: &str) -> Result<SecretLookupOutcome, String> {
        let token = self
            .token_provider
            .get_token(KEYVAULT_SCOPE)
            .await
            .map_err(|e| format!("key vault token error: {e}"))?;
        let url = format!(
            "{}/secrets/{}?api-version=7.4",
            self.vault_uri,
            urlencoding::encode(secret_name)
        );
        let resp = self
            .http
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| format!("key vault GET secret failed: {e}"))?;

        let status = resp.status();
        if status == StatusCode::NOT_FOUND {
            return Ok(SecretLookupOutcome::NotFound);
        }
        if !status.is_success() {
            return Err(format!("key vault returned HTTP {status}"));
        }

        let bundle: KeyVaultSecretBundle = resp
            .json()
            .await
            .map_err(|e| format!("invalid key vault secret bundle JSON: {e}"))?;
        Ok(SecretLookupOutcome::Found(bundle.value))
    }
}

pub struct ConfigurableEmailSender {
    mode: String,
    keyvault: Option<KeyVaultSecretClient>,
    secret_name: String,
    from_address: String,
    authorized_recipients: HashSet<String>,
    http: reqwest::Client,
}

#[derive(Deserialize)]
struct ResendResponse {
    id: String,
}

impl ConfigurableEmailSender {
    pub fn new(
        mode: String,
        keyvault: Option<KeyVaultSecretClient>,
        secret_name: String,
        from_address: String,
        authorized_recipients: HashSet<String>,
    ) -> Self {
        Self {
            mode,
            keyvault,
            secret_name,
            from_address,
            authorized_recipients,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("HTTP client"),
        }
    }
}

#[async_trait]
impl EmailDeliveryPort for ConfigurableEmailSender {
    fn configured_mode(&self) -> &str {
        &self.mode
    }

    async fn send_notification(
        &self,
        request: &NotificationRequestV1,
    ) -> Result<EmailDeliveryResult, DeliveryError> {
        let is_synthetic = request.synthetic_probe.unwrap_or(false);
        // Safety rule: synthetic probes or explicit fake mode NEVER call external email APIs.
        if is_synthetic || self.mode.eq_ignore_ascii_case("fake") {
            info!(
                event_id = %request.event_id,
                correlation_id = %request.correlation_id,
                recipient = %redact_email(&request.recipient),
                synthetic_probe = is_synthetic,
                "FakeEmailSender recorded notification delivery without sending external email"
            );
            return Ok(EmailDeliveryResult {
                provider_message_id: format!("fake-msg-{}", request.event_id),
                delivery_mode: "fake".to_string(),
            });
        }

        if !self.mode.eq_ignore_ascii_case("resend") {
            return Err(DeliveryError::Terminal(format!(
                "unsupported EMAIL_DELIVERY_MODE '{}'",
                self.mode
            )));
        }

        let recipient_lower = request.recipient.trim().to_ascii_lowercase();
        if !self.authorized_recipients.contains(&recipient_lower) {
            return Err(DeliveryError::UnauthorizedRecipient);
        }

        let api_key = match std::env::var("RESEND_API_KEY")
            .ok()
            .filter(|v| !v.is_empty())
        {
            Some(k) => k,
            None => {
                let kv = self.keyvault.as_ref().ok_or_else(|| {
                    DeliveryError::Terminal(
                        "RESEND_API_KEY not set and KEYVAULT_URI not configured".to_string(),
                    )
                })?;
                match kv.get_secret(&self.secret_name).await {
                    Ok(SecretLookupOutcome::Found(secret)) => secret,
                    Ok(SecretLookupOutcome::NotFound) => {
                        return Err(DeliveryError::Terminal(format!(
                            "Resend secret '{}' not populated in Key Vault",
                            self.secret_name
                        )));
                    }
                    Err(e) => return Err(DeliveryError::Transient(e)),
                }
            }
        };

        let payload = serde_json::json!({
            "from": self.from_address,
            "to": [request.recipient],
            "subject": format!("Campus Device Loan: {} ({})", request.event_type, request.device_model),
            "text": format!(
                "Notification {} for device model {} processed.\nEvent ID: {}\nCorrelation ID: {}\n",
                request.event_type,
                request.device_model,
                request.event_id,
                request.correlation_id
            )
        });

        let resp = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(api_key)
            .header("Idempotency-Key", &request.event_id)
            .json(&payload)
            .send()
            .await
            .map_err(|e| DeliveryError::Transient(format!("resend request failed: {e}")))?;

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
            return Err(DeliveryError::Transient(format!("resend HTTP {status}")));
        }
        if !status.is_success() {
            return Err(DeliveryError::Terminal(format!("resend HTTP {status}")));
        }

        let parsed: ResendResponse = serde_json::from_str(&body)
            .map_err(|e| DeliveryError::Transient(format!("resend response parse error: {e}")))?;
        Ok(EmailDeliveryResult {
            provider_message_id: parsed.id,
            delivery_mode: "resend".to_string(),
        })
    }
}

/// Diagnostic helper used in `verify-isolation` mode to prove from inside the Rust worker
/// that the `notifications` managed identity is rejected by PostgreSQL.
pub async fn verify_postgres_access_denied(
    host: &str,
    port: u16,
    dbname: &str,
    user: &str,
    token_provider: &ManagedIdentityTokenProvider,
) -> Result<String, String> {
    let token = token_provider
        .get_token(POSTGRES_SCOPE)
        .await
        .map_err(|e| format!("failed to acquire ossrdbms token for isolation check: {e}"))?;

    let connector = TlsConnector::builder()
        .build()
        .map_err(|e| format!("failed to build TLS connector: {e}"))?;
    let tls = MakeTlsConnector::new(connector);

    let mut pg_cfg = PgConfig::new();
    pg_cfg.host(host);
    pg_cfg.port(port);
    pg_cfg.dbname(dbname);
    pg_cfg.user(user);
    pg_cfg.password(token);
    pg_cfg.ssl_mode(tokio_postgres::config::SslMode::Require);

    match pg_cfg.connect(tls).await {
        Ok(_) => Err(
            "SECURITY VIOLATION: notifications identity unexpectedly connected to PostgreSQL!"
                .to_string(),
        ),
        Err(err) => {
            info!(
                expected_denial = %err,
                "verified service boundary: PostgreSQL rejected notifications managed identity"
            );
            Ok(err.to_string())
        }
    }
}
