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
    pub models: BTreeMap<String, StaffInventoryModel>,
    pub reservations: BTreeMap<Uuid, Reservation>,
    pub subscriptions: BTreeMap<String, (AuthenticatedActor, SubscriptionReceipt)>,
    pub outbox: Vec<NotificationRequestV1>,
    pub fail_outbox: bool,
}

impl Data {
    fn sync_legacy_model_fields(&mut self) {
        if let Some(m) = self.models.get("model") {
            self.available = m.available_units;
            self.units = m
                .units
                .iter()
                .map(|u| (u.unit_id.clone(), u.status.clone()))
                .collect();
        }
    }
}

pub struct FakeStore {
    pub data: Mutex<Data>,
    pub clock: std::sync::Mutex<DateTime<Utc>>,
}
impl FakeStore {
    pub fn with_units(units: i32) -> Self {
        let unit_map: BTreeMap<String, String> = (0..units)
            .map(|i| (format!("unit-{i}"), "AVAILABLE".into()))
            .collect();
        let model_units: Vec<DeviceUnitSummary> = (0..units)
            .map(|i| DeviceUnitSummary {
                unit_id: format!("unit-{i}"),
                serial_number: format!("SN-MODEL-{i}"),
                status: "AVAILABLE".into(),
            })
            .collect();
        let mut models = BTreeMap::new();
        models.insert(
            "model".into(),
            StaffInventoryModel {
                id: "model".into(),
                name: "Test Laptop".into(),
                category: "Laptop".into(),
                description: "Test".into(),
                loan_duration_days: 14,
                total_units: units,
                available_units: units,
                image_url: None,
                image_alt: None,
                specifications: vec![],
                units: model_units,
            },
        );
        Self {
            data: Mutex::new(Data {
                available: units,
                units: unit_map,
                models,
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
            let mut items: Vec<DeviceModelSummary> = data
                .models
                .values()
                .map(|m| DeviceModelSummary {
                    id: m.id.clone(),
                    name: m.name.clone(),
                    category: m.category.clone(),
                    description: m.description.clone(),
                    loan_duration_days: m.loan_duration_days,
                    total_units: m.total_units,
                    available_units: m.available_units,
                    image_url: m.image_url.clone(),
                    image_alt: m.image_alt.clone(),
                    specifications: m.specifications.clone(),
                })
                .collect();
            items.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));
            Ok(items)
        })
    }
    fn staff_inventory(&self) -> BoxFuture<'_, Result<Vec<StaffInventoryModel>, ApplicationError>> {
        Box::pin(async {
            let data = self.data.lock().await;
            let mut items: Vec<StaffInventoryModel> = data.models.values().cloned().collect();
            items.sort_by(|a, b| a.category.cmp(&b.category).then(a.name.cmp(&b.name)));
            Ok(items)
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
            let m = self
                .models
                .get(&id)
                .ok_or_else(|| ApplicationError::NotFound(id.clone()))?;
            Ok(LockedModel {
                id,
                name: m.name.clone(),
                available_units: m.available_units,
                loan_duration_days: m.loan_duration_days,
            })
        })
    }
    fn available_unit(
        &mut self,
        model: String,
    ) -> BoxFuture<'_, Result<Option<String>, ApplicationError>> {
        Box::pin(async move {
            Ok(self.models.get(&model).and_then(|m| {
                m.units
                    .iter()
                    .find(|u| u.status == "AVAILABLE")
                    .map(|u| u.unit_id.clone())
            }))
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
            if let Some(m) = self.models.get_mut(&r.model_id) {
                m.available_units -= 1;
                if let Some(u) = m.units.iter_mut().find(|u| u.unit_id == r.unit_id) {
                    u.status = "RESERVED".into();
                }
            }
            self.reservations.insert(r.reservation_id, r);
            self.sync_legacy_model_fields();
            Ok(())
        })
    }
    fn upsert_subscription(
        &mut self,
        actor: AuthenticatedActor,
        receipt: SubscriptionReceipt,
    ) -> BoxFuture<'_, Result<SubscriptionReceipt, ApplicationError>> {
        Box::pin(async move {
            let key = format!("{}:{}", receipt.model_id, actor.subject);
            let entry = self
                .subscriptions
                .entry(key)
                .or_insert((actor.clone(), receipt));
            entry.0.email = actor.email;
            Ok(entry.1.clone())
        })
    }
    fn save_collection(&mut self, r: Reservation) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            if let Some(m) = self.models.get_mut(&r.model_id) {
                if let Some(u) = m.units.iter_mut().find(|u| u.unit_id == r.unit_id) {
                    u.status = "COLLECTED".into();
                }
            }
            self.reservations.insert(r.reservation_id, r);
            self.sync_legacy_model_fields();
            Ok(())
        })
    }
    fn active_subscribers(
        &mut self,
        model: String,
    ) -> BoxFuture<'_, Result<Vec<String>, ApplicationError>> {
        Box::pin(async move {
            Ok(self
                .subscriptions
                .values()
                .filter(|(_, r)| r.model_id == model && r.status == "ACTIVE")
                .map(|(a, _)| a.email.clone())
                .collect())
        })
    }
    fn save_return(&mut self, r: Reservation) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            if let Some(m) = self.models.get_mut(&r.model_id) {
                m.available_units += 1;
                if let Some(u) = m.units.iter_mut().find(|u| u.unit_id == r.unit_id) {
                    u.status = "AVAILABLE".into();
                }
            }
            let model_id = r.model_id.clone();
            self.reservations.insert(r.reservation_id, r);
            self.subscriptions
                .retain(|_, (_, sub)| sub.model_id != model_id);
            self.sync_legacy_model_fields();
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
    fn insert_model_with_units(
        &mut self,
        input: CreateDeviceModelInput,
        _now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<StaffInventoryModel, ApplicationError>> {
        Box::pin(async move {
            if self.models.contains_key(&input.id) {
                return Err(ApplicationError::Domain(DomainError::AlreadyExists(
                    format!("device model '{}' already exists", input.id),
                )));
            }
            let existing_serials: std::collections::HashSet<String> = self
                .models
                .values()
                .flat_map(|m| m.units.iter().map(|u| u.serial_number.to_ascii_lowercase()))
                .collect();
            for u in &input.units {
                if existing_serials.contains(&u.serial_number.to_ascii_lowercase()) {
                    return Err(ApplicationError::Domain(DomainError::AlreadyExists(
                        "a unit with this serial number already exists".into(),
                    )));
                }
            }
            let total_units = input.units.len() as i32;
            let available_units = input
                .units
                .iter()
                .filter(|u| u.status == "AVAILABLE")
                .count() as i32;
            let units = input
                .units
                .into_iter()
                .enumerate()
                .map(|(idx, u)| DeviceUnitSummary {
                    unit_id: format!("campus-{}-{:03}", input.id, idx + 1),
                    serial_number: u.serial_number,
                    status: u.status,
                })
                .collect();
            let model = StaffInventoryModel {
                id: input.id.clone(),
                name: input.name,
                category: input.category,
                description: input.description,
                loan_duration_days: input.loan_duration_days,
                total_units,
                available_units,
                image_url: Some(input.image_url),
                image_alt: Some(input.image_alt),
                specifications: input.specifications,
                units,
            };
            self.models.insert(input.id, model.clone());
            self.sync_legacy_model_fields();
            Ok(model)
        })
    }
    fn delete_model(&mut self, model_id: String) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            let model = self
                .models
                .get(&model_id)
                .ok_or_else(|| ApplicationError::NotFound(model_id.clone()))?;
            if model
                .units
                .iter()
                .any(|u| u.status == "RESERVED" || u.status == "COLLECTED")
            {
                return Err(ApplicationError::Domain(DomainError::ResourceInUse(
                    "cannot remove a model while units are reserved or collected".into(),
                )));
            }
            if self.reservations.values().any(|r| r.model_id == model_id) {
                return Err(ApplicationError::Domain(DomainError::ResourceInUse(
                    "cannot remove a model with past loan records; move its units to maintenance instead".into(),
                )));
            }
            self.subscriptions
                .retain(|_, (_, sub)| sub.model_id != model_id);
            self.models.remove(&model_id);
            self.sync_legacy_model_fields();
            Ok(())
        })
    }
    fn insert_unit(
        &mut self,
        model_id: String,
        input: NewDeviceUnitInput,
        _now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<DeviceUnitSummary, ApplicationError>> {
        Box::pin(async move {
            let serial_taken = self.models.values().any(|m| {
                m.units
                    .iter()
                    .any(|u| u.serial_number.eq_ignore_ascii_case(&input.serial_number))
            });
            if serial_taken {
                return Err(ApplicationError::Domain(DomainError::AlreadyExists(
                    "a unit with this serial number already exists".into(),
                )));
            }
            let model = self
                .models
                .get_mut(&model_id)
                .ok_or_else(|| ApplicationError::NotFound(model_id.clone()))?;
            let unit = DeviceUnitSummary {
                unit_id: format!(
                    "unit-{model_id}-{}",
                    &Uuid::new_v4().simple().to_string()[..8]
                ),
                serial_number: input.serial_number,
                status: input.status,
            };
            model.total_units += 1;
            if unit.status == "AVAILABLE" {
                model.available_units += 1;
            }
            model.units.push(unit.clone());
            self.sync_legacy_model_fields();
            Ok(unit)
        })
    }
    fn lock_unit(
        &mut self,
        unit_id: String,
    ) -> BoxFuture<'_, Result<(LockedModel, DeviceUnitSummary), ApplicationError>> {
        Box::pin(async move {
            let found = self.models.values().find_map(|m| {
                m.units.iter().find(|u| u.unit_id == unit_id).map(|u| {
                    (
                        LockedModel {
                            id: m.id.clone(),
                            name: m.name.clone(),
                            available_units: m.available_units,
                            loan_duration_days: m.loan_duration_days,
                        },
                        u.clone(),
                    )
                })
            });
            found.ok_or_else(|| ApplicationError::NotFound(format!("device unit {unit_id}")))
        })
    }
    fn update_unit_status(
        &mut self,
        model_id: String,
        unit_id: String,
        from_status: String,
        to_status: String,
        _now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            let model = self
                .models
                .get_mut(&model_id)
                .ok_or(ApplicationError::NotFound(model_id))?;
            if let Some(u) = model.units.iter_mut().find(|u| u.unit_id == unit_id) {
                u.status = to_status.clone();
            }
            match (from_status.as_str(), to_status.as_str()) {
                ("MAINTENANCE", "AVAILABLE") => model.available_units += 1,
                ("AVAILABLE", "MAINTENANCE") => model.available_units -= 1,
                _ => {}
            }
            self.sync_legacy_model_fields();
            Ok(())
        })
    }
    fn mark_subscribers_notified(
        &mut self,
        model_id: String,
        _now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            for (_, sub) in self.subscriptions.values_mut() {
                if sub.model_id == model_id && sub.status == "ACTIVE" {
                    sub.status = "NOTIFIED".into();
                }
            }
            Ok(())
        })
    }
    fn delete_unit(
        &mut self,
        model_id: String,
        unit_id: String,
        was_available: bool,
        _now: DateTime<Utc>,
    ) -> BoxFuture<'_, Result<(), ApplicationError>> {
        Box::pin(async move {
            if self.reservations.values().any(|r| r.unit_id == unit_id) {
                return Err(ApplicationError::Domain(DomainError::ResourceInUse(
                    "cannot remove a unit with past loan records; move it to maintenance instead"
                        .into(),
                )));
            }
            let model = self
                .models
                .get_mut(&model_id)
                .ok_or(ApplicationError::NotFound(model_id))?;
            model.units.retain(|u| u.unit_id != unit_id);
            model.total_units -= 1;
            if was_available {
                model.available_units -= 1;
            }
            self.sync_legacy_model_fields();
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

/// Fake verifier: bearer `subject:role` authenticates; `keys-down` simulates a JWKS outage.
pub struct FakeVerifier {
    pub configured: bool,
}

impl AccessTokenVerifierPort for FakeVerifier {
    fn is_configured(&self) -> bool {
        self.configured
    }

    fn verify_bearer<'a>(
        &'a self,
        auth_header: Option<&'a str>,
    ) -> BoxFuture<'a, Result<AuthenticatedActor, AuthVerifyError>> {
        Box::pin(async move {
            if !self.configured {
                return Err(AuthVerifyError::NotConfigured);
            }
            let token = auth_header
                .and_then(|h| h.strip_prefix("Bearer "))
                .filter(|t| !t.is_empty())
                .ok_or(AuthVerifyError::MissingBearerToken)?;
            if token == "keys-down" {
                return Err(AuthVerifyError::KeysUnavailable("fake".into()));
            }
            let (subject, role) = token
                .split_once(':')
                .ok_or_else(|| AuthVerifyError::InvalidToken("fake".into()))?;
            let role = match role {
                "staff" => Role::Staff,
                "student" => Role::Student,
                _ => return Err(AuthVerifyError::InvalidToken("fake role".into())),
            };
            Ok(AuthenticatedActor {
                subject: subject.into(),
                email: format!("{subject}@example.invalid"),
                roles: vec![role],
            })
        })
    }
}
