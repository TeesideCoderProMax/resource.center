//! Transactional in-memory storage. All decisions use the production application use cases.
use crate::application::*;
use crate::domain::*;
use chrono::{DateTime, Utc};
use rc_shared::NotificationRequestV1;
use std::collections::BTreeMap;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct Data {
    pub available: i32,
    pub units: BTreeMap<String, String>,
    pub reservations: BTreeMap<Uuid, Reservation>,
    pub subscriptions: BTreeMap<String, (AuthenticatedActor, SubscriptionReceipt)>,
    pub outbox: Vec<NotificationRequestV1>,
    pub fail_outbox: bool,
}

pub struct FakeStore {
    pub data: Mutex<Data>,
    pub clock: std::sync::Mutex<DateTime<Utc>>,
}
impl FakeStore {
    pub fn with_units(units: i32) -> Self {
        Self {
            data: Mutex::new(Data {
                available: units,
                units: (0..units)
                    .map(|i| (format!("unit-{i}"), "AVAILABLE".into()))
                    .collect(),
                reservations: BTreeMap::new(),
                subscriptions: BTreeMap::new(),
                outbox: vec![],
                fail_outbox: false,
            }),
            clock: std::sync::Mutex::new("2026-11-01T12:00:00Z".parse().unwrap()),
        }
    }
}
impl LoanStorePort for FakeStore {
    fn now(&self) -> DateTime<Utc> {
        *self.clock.lock().unwrap()
    }
    fn readiness(&self) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async { Ok(()) })
    }
    fn catalogue(&self) -> BoxFuture<'_, Result<Vec<DeviceModelSummary>, ApplicationError>> {
        Box::pin(async {
            let data = self.data.lock().await;
            Ok(vec![DeviceModelSummary {
                id: "model".into(),
                name: "Test Laptop".into(),
                category: "Laptop".into(),
                description: "Test".into(),
                loan_duration_days: 14,
                image_url: None,
                image_alt: None,
                specifications: vec![],
                total_units: data.units.len() as i32,
                available_units: data.available,
            }])
        })
    }
    fn reservations(
        &self,
        subject: Option<String>,
    ) -> BoxFuture<'_, Result<Vec<Reservation>, ApplicationError>> {
        Box::pin(async move {
            Ok(self
                .data
                .lock()
                .await
                .reservations
                .values()
                .filter(|r| subject.as_ref().is_none_or(|s| s == &r.student_sub))
                .cloned()
                .collect())
        })
    }
    fn transact<T: Send + 'static>(
        &self,
        action: TransactionAction<T>,
    ) -> BoxFuture<'_, Result<T, ApplicationError>> {
        Box::pin(async move {
            let mut data = self.data.lock().await;
            let mut draft = data.clone();
            let result = action(&mut draft).await?;
            *data = draft;
            Ok(result)
        })
    }
}
impl LoanTransactionPort for Data {
    fn lock_model(&mut self, id: String) -> BoxFuture<'_, Result<LockedModel, ApplicationError>> {
        Box::pin(async move {
            if id != "model" {
                return Err(ApplicationError::NotFound(id));
            }
            Ok(LockedModel {
                id,
                name: "Test Laptop".into(),
                available_units: self.available,
                loan_duration_days: 14,
            })
        })
    }
    fn available_unit(
        &mut self,
        _: String,
    ) -> BoxFuture<'_, Result<Option<String>, ApplicationError>> {
        Box::pin(async {
            Ok(self
                .units
                .iter()
                .find(|(_, status)| status.as_str() == "AVAILABLE")
                .map(|(id, _)| id.clone()))
        })
    }
    fn lock_reservation(
        &mut self,
        id: Uuid,
    ) -> BoxFuture<'_, Result<(LockedModel, Reservation), ApplicationError>> {
        Box::pin(async move {
            let r = self
                .reservations
                .get(&id)
                .cloned()
                .ok_or_else(|| ApplicationError::NotFound(id.to_string()))?;
            let model = self.lock_model(r.model_id.clone()).await?;
            Ok((model, r))
        })
    }
    fn insert_reservation(
        &mut self,
        r: Reservation,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            self.available -= 1;
            self.units.insert(r.unit_id.clone(), "RESERVED".into());
            self.reservations.insert(r.reservation_id, r);
            Ok(())
        })
    }
    fn upsert_subscription(
        &mut self,
        actor: AuthenticatedActor,
        receipt: SubscriptionReceipt,
    ) -> BoxFuture<'_, Result<SubscriptionReceipt, ApplicationError>> {
        Box::pin(async move {
            let entry = self
                .subscriptions
                .entry(actor.subject.clone())
                .or_insert((actor.clone(), receipt));
            entry.0.email = actor.email;
            Ok(entry.1.clone())
        })
    }
    fn save_collection(&mut self, r: Reservation) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            self.units.insert(r.unit_id.clone(), "COLLECTED".into());
            self.reservations.insert(r.reservation_id, r);
            Ok(())
        })
    }
    fn active_subscribers(
        &mut self,
        _: String,
    ) -> BoxFuture<'_, Result<Vec<String>, ApplicationError>> {
        Box::pin(async {
            Ok(self
                .subscriptions
                .values()
                .map(|(a, _)| a.email.clone())
                .collect())
        })
    }
    fn save_return(&mut self, r: Reservation) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            self.available += 1;
            self.units.insert(r.unit_id.clone(), "AVAILABLE".into());
            self.reservations.insert(r.reservation_id, r);
            self.subscriptions.clear();
            Ok(())
        })
    }
    fn append_outbox(
        &mut self,
        events: Vec<NotificationRequestV1>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            if self.fail_outbox {
                return Err(ApplicationError::Persistence(
                    "forced outbox failure".into(),
                ));
            }
            self.outbox.extend(events);
            Ok(())
        })
    }
}

pub fn student(id: &str) -> AuthenticatedActor {
    AuthenticatedActor {
        subject: id.into(),
        email: format!("{id}@example.invalid"),
        roles: vec![Role::Student],
    }
}
pub fn staff() -> AuthenticatedActor {
    AuthenticatedActor {
        roles: vec![Role::Staff],
        ..student("staff")
    }
}

#[tokio::test]
async fn rules_due_dates_roles_ownership_duplicates_and_rollback() {
    let store = FakeStore::with_units(1);
    assert!(matches!(
        store
            .subscribe_availability(student("wait"), "model".into())
            .await,
        Err(ApplicationError::Domain(
            DomainError::UnitsCurrentlyAvailable(_)
        ))
    ));
    let r = store
        .reserve_device(student("owner"), "model".into(), 2, "corr".into())
        .await
        .unwrap();
    assert_eq!((r.due_at - r.reserved_at).num_hours(), 48);
    assert!(store
        .list_reservations(student("other"))
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        store
            .list_reservations(student("owner"))
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(store.list_reservations(staff()).await.unwrap().len(), 1);
    assert!(store
        .list_reservations(AuthenticatedActor {
            roles: vec![],
            ..student("none")
        })
        .await
        .is_err());
    assert!(matches!(
        store
            .record_collection(student("owner"), r.reservation_id, "c".into())
            .await,
        Err(ApplicationError::Domain(DomainError::ForbiddenRole(_)))
    ));
    assert!(matches!(
        store
            .record_return(staff(), r.reservation_id, "c".into())
            .await,
        Err(ApplicationError::Domain(
            DomainError::InvalidTransition { .. }
        ))
    ));
    *store.clock.lock().unwrap() += chrono::Duration::hours(5);
    store.data.lock().await.fail_outbox = true;
    assert!(store
        .record_collection(staff(), r.reservation_id, "c".into())
        .await
        .is_err());
    assert_eq!(
        store.data.lock().await.reservations[&r.reservation_id].status,
        "RESERVED"
    );
    store.data.lock().await.fail_outbox = false;
    let c = store
        .record_collection(staff(), r.reservation_id, "c".into())
        .await
        .unwrap();
    assert_eq!(c.due_at, r.due_at);
    assert_eq!(c.collected_at - r.reserved_at, chrono::Duration::hours(5));
    assert!(store
        .record_collection(staff(), r.reservation_id, "c".into())
        .await
        .is_err());
    let a = store
        .subscribe_availability(student("wait1"), "model".into())
        .await
        .unwrap();
    let b = store
        .subscribe_availability(student("wait1"), "model".into())
        .await
        .unwrap();
    assert_eq!(a.subscription_id, b.subscription_id);
    assert_eq!(a.created_at, b.created_at);
    store
        .subscribe_availability(student("wait2"), "model".into())
        .await
        .unwrap();
    store.data.lock().await.fail_outbox = true;
    assert!(store
        .record_return(staff(), r.reservation_id, "c".into())
        .await
        .is_err());
    {
        let data = store.data.lock().await;
        assert_eq!(data.available, 0);
        assert_eq!(data.subscriptions.len(), 2);
        assert_eq!(data.outbox.len(), 1);
    }
    store.data.lock().await.fail_outbox = false;
    let returned = store
        .record_return(staff(), r.reservation_id, "c".into())
        .await
        .unwrap();
    assert_eq!(returned.outbox_event_ids.len(), 3);
    assert!(store
        .record_return(staff(), r.reservation_id, "c".into())
        .await
        .is_err());
    let data = store.data.lock().await;
    assert_eq!(data.available, 1);
    assert_eq!(data.outbox.len(), 4);
    for event in &data.outbox {
        event.validate().unwrap();
    }
}

#[tokio::test]
async fn fake_last_unit_has_one_winner() {
    let store = std::sync::Arc::new(FakeStore::with_units(1));
    let mut tasks = vec![];
    for i in 0..5 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store
                .reserve_device(student(&format!("s{i}")), "model".into(), 2, "c".into())
                .await
        }));
    }
    let mut winners = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(_) => winners += 1,
            Err(ApplicationError::Domain(DomainError::NoAvailableUnits(_))) => (),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(winners, 1);
}

#[tokio::test]
async fn fake_allocates_distinct_units_and_restores_only_returned_unit() {
    let store = FakeStore::with_units(2);
    let first = store
        .reserve_device(student("one"), "model".into(), 2, "c".into())
        .await
        .unwrap();
    let second = store
        .reserve_device(student("two"), "model".into(), 2, "c".into())
        .await
        .unwrap();
    assert_ne!(first.unit_id, second.unit_id);
    store
        .record_collection(staff(), first.reservation_id, "c".into())
        .await
        .unwrap();
    store
        .record_return(staff(), first.reservation_id, "c".into())
        .await
        .unwrap();
    let third = store
        .reserve_device(student("three"), "model".into(), 2, "c".into())
        .await
        .unwrap();
    assert_eq!(first.unit_id, third.unit_id);
    assert_eq!(store.list_catalogue().await.unwrap()[0].available_units, 0);
}

#[tokio::test]
async fn selected_periods_set_due_dates_and_invalid_periods_leave_no_writes() {
    let store = FakeStore::with_units(3);
    for days in [0, -1, 15, i32::MAX] {
        assert!(store
            .reserve_device(student("bad"), "model".into(), days, "invalid".into())
            .await
            .is_err());
    }
    {
        let data = store.data.lock().await;
        assert_eq!(data.available, 3);
        assert!(data.reservations.is_empty());
        assert!(data.outbox.is_empty());
    }
    for days in [1, 14] {
        let receipt = store
            .reserve_device(student("valid"), "model".into(), days, "period".into())
            .await
            .unwrap();
        assert_eq!(
            receipt.due_at - receipt.reserved_at,
            chrono::Duration::days(i64::from(days))
        );
    }
    assert!(compute_due_at(store.now(), 4, 3).is_err());
}
