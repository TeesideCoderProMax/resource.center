//! Opt-in checks against real Cosmos. Uses unique synthetic IDs and no email.
use crate::application::*;
use crate::domain::NotificationJobDocument;
use crate::infrastructure::CosmosRestJobStore;
use chrono::Utc;
use rc_shared::{
    ManagedIdentityTokenProvider, NotificationRequestV1, CONTRACT_VERSION_V1,
    EVENT_TYPE_AVAILABILITY_REQUESTED,
};
use std::sync::Arc;

#[tokio::test]
#[ignore = "requires real Cosmos data access; run make backend-test-cosmos"]
async fn real_cosmos_claim_renew_finalize_conflicts() {
    let store = CosmosRestJobStore::new(
        std::env::var("COSMOS_ENDPOINT").expect("COSMOS_ENDPOINT"),
        std::env::var("COSMOS_DATABASE").unwrap_or_else(|_| "notifications".into()),
        std::env::var("COSMOS_CONTAINER").unwrap_or_else(|_| "jobs".into()),
        Arc::new(
            ManagedIdentityTokenProvider::new(std::env::var("AZURE_CLIENT_ID").unwrap_or_default())
                .unwrap(),
        ),
    );
    let req = NotificationRequestV1 {
        event_id: format!("phase2-check-{}", uuid::Uuid::new_v4()),
        correlation_id: "phase2-cosmos-check".into(),
        event_type: EVENT_TYPE_AVAILABILITY_REQUESTED.into(),
        version: CONTRACT_VERSION_V1,
        occurred_at: Utc::now(),
        recipient: "synthetic@example.invalid".into(),
        device_model: "Synthetic".into(),
        synthetic_probe: Some(true),
    };
    let candidate = NotificationJobDocument::new_claimed(&req, "fake", Utc::now());
    let (a, b) = tokio::join!(store.create_job(&candidate), store.create_job(&candidate));
    let first = match (a.unwrap(), b.unwrap()) {
        (CreateJobOutcome::Created(doc), CreateJobOutcome::AlreadyExists)
        | (CreateJobOutcome::AlreadyExists, CreateJobOutcome::Created(doc)) => doc,
        other => panic!("expected one creator: {other:?}"),
    };
    let mut renewal = first.document.clone();
    renewal.lease_until = Some(Utc::now() + chrono::Duration::seconds(90));
    let (a, b) = tokio::join!(
        store.replace_job(&renewal, &first.etag),
        store.replace_job(&renewal, &first.etag)
    );
    let renewed = match (a.unwrap(), b.unwrap()) {
        (ReplaceJobOutcome::Replaced(doc), ReplaceJobOutcome::PreconditionFailed)
        | (ReplaceJobOutcome::PreconditionFailed, ReplaceJobOutcome::Replaced(doc)) => doc,
        other => panic!("expected one renewer: {other:?}"),
    };
    let mut completed = renewed.document.clone();
    completed.mark_delivered("synthetic-no-send".into(), "fake", Utc::now());
    assert_eq!(
        store.replace_job(&completed, &first.etag).await.unwrap(),
        ReplaceJobOutcome::PreconditionFailed
    );
    let (a, b) = tokio::join!(
        store.replace_job(&completed, &renewed.etag),
        store.replace_job(&completed, &renewed.etag)
    );
    assert!(matches!(
        (a.unwrap(), b.unwrap()),
        (
            ReplaceJobOutcome::Replaced(_),
            ReplaceJobOutcome::PreconditionFailed
        ) | (
            ReplaceJobOutcome::PreconditionFailed,
            ReplaceJobOutcome::Replaced(_)
        )
    ));
    assert!(store
        .get_job(&req.event_id)
        .await
        .unwrap()
        .unwrap()
        .document
        .is_terminal());
    store.delete_job(&req.event_id).await.unwrap();
}
