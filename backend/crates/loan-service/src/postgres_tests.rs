//! Real PostgreSQL checks run only against the disposable database from test-postgres.sh.
use crate::application::*;
use crate::domain::DomainError;
use crate::infrastructure::{PostgresAdapter, PostgresConfig};
use crate::test_support::{staff, student};
use chrono::{DateTime, Utc};
use uuid::Uuid;

fn adapter(database: &str) -> PostgresAdapter {
    PostgresAdapter::new(
        PostgresConfig {
            host: "localhost".into(),
            port: std::env::var("RC_TEST_PG_PORT")
                .expect("run make backend-test-postgres")
                .parse()
                .unwrap(),
            database: database.into(),
            user: "postgres".into(),
            static_password: Some("local-phase1-test".into()),
            max_connections: 8,
            publisher_role_name: None,
        },
        None,
    )
}

async fn database(name: &str) -> PostgresAdapter {
    let admin = adapter("postgres");
    let (client, _permit) = admin.connect_client().await.unwrap();
    client
        .batch_execute(&format!("CREATE DATABASE {name}"))
        .await
        .unwrap();
    adapter(name)
}

async fn seed(pg: &PostgresAdapter) {
    let (client, _permit) = pg.connect_client().await.unwrap();
    client.batch_execute("INSERT INTO loan.device_models(id,name,category,description,total_units,available_units) VALUES ('model','Test Laptop','Laptop','test',1,1); INSERT INTO loan.device_units(unit_id,model_id,serial_number,status) VALUES ('unit','model','serial','AVAILABLE');").await.unwrap();
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run make backend-test-postgres"]
async fn postgres_tests_migrations_transactions_contention_and_recovery() {
    let pg = database("phase1_fresh").await;
    pg.run_migrations(false).await.unwrap();
    pg.run_migrations(false).await.unwrap(); // versioned upgrade is repeatable
    seed(&pg).await;
    let mut tasks = vec![];
    for i in 0..5 {
        let pg = pg.clone();
        tasks.push(tokio::spawn(async move {
            pg.reserve_device(
                student(&format!("s{i}")),
                "model".into(),
                2,
                "reserve".into(),
            )
            .await
        }));
    }
    let mut winners = vec![];
    for task in tasks {
        match task.await.unwrap() {
            Ok(r) => winners.push(r),
            Err(ApplicationError::Domain(DomainError::NoAvailableUnits(_))) => (),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(winners.len(), 1);
    let r = &winners[0];
    assert_eq!((r.due_at - r.reserved_at).num_seconds(), 172800);
    assert!(pg
        .record_return(staff(), r.reservation_id, "early".into())
        .await
        .is_err());
    assert!(pg
        .record_collection(student("student"), r.reservation_id, "role".into())
        .await
        .is_err());
    assert!(pg
        .list_reservations(student("stranger"))
        .await
        .unwrap()
        .is_empty());
    assert_eq!(pg.list_reservations(staff()).await.unwrap().len(), 1);
    let a = pg
        .subscribe_availability(student("wait1"), "model".into())
        .await
        .unwrap();
    let b = pg
        .subscribe_availability(student("wait1"), "model".into())
        .await
        .unwrap();
    assert_eq!(a.subscription_id, b.subscription_id);
    assert_eq!(a.created_at, b.created_at);
    pg.subscribe_availability(student("wait2"), "model".into())
        .await
        .unwrap();
    let (client, _permit) = pg.connect_client().await.unwrap();
    // This trigger fails AFTER reservation and unit writes, at the outbox insert.
    client.batch_execute("CREATE FUNCTION loan.fail_outbox() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'forced outbox failure'; END $$; CREATE TRIGGER fail_outbox BEFORE INSERT ON loan.outbox_events FOR EACH ROW EXECUTE FUNCTION loan.fail_outbox();").await.unwrap();
    assert!(pg
        .record_collection(staff(), r.reservation_id, "rollback".into())
        .await
        .is_err());
    let row = client.query_one("SELECT r.status,u.status,r.collected_at,(SELECT count(*) FROM loan.outbox_events) FROM loan.reservations r JOIN loan.device_units u USING(unit_id)", &[]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "RESERVED");
    assert_eq!(row.get::<_, String>(1), "RESERVED");
    assert!(row.get::<_, Option<DateTime<Utc>>>(2).is_none());
    assert_eq!(row.get::<_, i64>(3), 0);
    client
        .batch_execute("DROP TRIGGER fail_outbox ON loan.outbox_events")
        .await
        .unwrap();
    let c = pg
        .record_collection(staff(), r.reservation_id, "collect".into())
        .await
        .unwrap();
    // PostgreSQL timestamps retain microseconds, while chrono may include nanoseconds.
    assert_eq!(c.due_at.timestamp_micros(), r.due_at.timestamp_micros());
    assert!(pg
        .record_collection(staff(), r.reservation_id, "repeat".into())
        .await
        .is_err());
    client.batch_execute("CREATE TRIGGER fail_outbox BEFORE INSERT ON loan.outbox_events FOR EACH ROW EXECUTE FUNCTION loan.fail_outbox();").await.unwrap();
    assert!(pg
        .record_return(staff(), r.reservation_id, "rollback".into())
        .await
        .is_err());
    let row = client.query_one("SELECT available_units,(SELECT count(*) FROM loan.availability_subscriptions WHERE status='ACTIVE'),(SELECT count(*) FROM loan.outbox_events),(SELECT status FROM loan.reservations LIMIT 1) FROM loan.device_models", &[]).await.unwrap();
    assert_eq!(row.get::<_, i32>(0), 0);
    assert_eq!(row.get::<_, i64>(1), 2);
    assert_eq!(row.get::<_, i64>(2), 1);
    assert_eq!(row.get::<_, String>(3), "COLLECTED");
    client
        .batch_execute("DROP TRIGGER fail_outbox ON loan.outbox_events")
        .await
        .unwrap();
    // Concurrent returns: exactly one transition and one set of events.
    let (a, b) = tokio::join!(
        pg.record_return(staff(), r.reservation_id, "return1".into()),
        pg.record_return(staff(), r.reservation_id, "return2".into())
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let result = a.or(b).unwrap();
    assert_eq!(result.outbox_event_ids.len(), 3);
    assert_eq!(pg.list_catalogue().await.unwrap()[0].available_units, 1);
    let rows = client
        .query("SELECT payload FROM loan.outbox_events", &[])
        .await
        .unwrap();
    assert_eq!(rows.len(), 4);
    for row in rows {
        let event: rc_shared::NotificationRequestV1 = serde_json::from_value(row.get(0)).unwrap();
        event.validate().unwrap();
    }
    // Re-subscription after a notification must survive a second return.
    let second = pg
        .reserve_device(student("owner2"), "model".into(), 2, "second".into())
        .await
        .unwrap();
    pg.subscribe_availability(student("wait1"), "model".into())
        .await
        .unwrap();
    pg.record_collection(staff(), second.reservation_id, "second".into())
        .await
        .unwrap();
    pg.record_return(staff(), second.reservation_id, "second".into())
        .await
        .unwrap();
    let count: i64 = client.query_one("SELECT count(*) FROM loan.availability_subscriptions WHERE student_sub='wait1' AND status='NOTIFIED'", &[]).await.unwrap().get(0);
    assert_eq!(count, 2);
    println!("PASS fresh migration/re-run, five-way contention, due date, roles/ownership, duplicate subscription, partial-write rollback (collect and return), concurrent double return, multi-recipient v1 outbox, repeat subscription cycle");

    let legacy = database("phase1_legacy").await;
    let (client, _permit) = legacy.connect_client().await.unwrap();
    client.batch_execute("CREATE SCHEMA loan").await.unwrap();
    client
        .batch_execute(include_str!("../migrations/0001_initial_loan_schema.sql"))
        .await
        .unwrap();
    seed(&legacy).await;
    let preserved = Uuid::new_v4();
    let backfill = Uuid::new_v4();
    let invalid = Uuid::new_v4();
    for (id, due) in [
        (preserved, Some("2026-11-05T12:00:00Z")),
        (backfill, None),
        (invalid, Some("2026-10-30T12:00:00Z")),
    ] {
        let due: Option<DateTime<Utc>> = due.map(|v| v.parse().unwrap());
        client.execute("INSERT INTO loan.reservations(reservation_id,model_id,unit_id,student_sub,student_email,status,reserved_at,due_at,correlation_id) VALUES($1,'model','unit','old','old@example.invalid','RETURNED','2026-11-01T12:00:00Z',$2,'legacy')", &[&id,&due]).await.unwrap();
    }
    assert!(legacy.run_migrations(false).await.is_err());
    assert!(client
        .query_one(
            "SELECT due_at FROM loan.reservations WHERE reservation_id=$1",
            &[&backfill]
        )
        .await
        .unwrap()
        .get::<_, Option<DateTime<Utc>>>(0)
        .is_none());
    // Test fixture repair only: the migration must never silently repair invalid dates.
    client
        .execute(
            "UPDATE loan.reservations SET due_at='2026-11-03T12:00:00Z' WHERE reservation_id=$1",
            &[&invalid],
        )
        .await
        .unwrap();
    legacy.run_migrations(false).await.unwrap();
    let due: DateTime<Utc> = client
        .query_one(
            "SELECT due_at FROM loan.reservations WHERE reservation_id=$1",
            &[&preserved],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(due.to_rfc3339(), "2026-11-05T12:00:00+00:00");
    let due: DateTime<Utc> = client
        .query_one(
            "SELECT due_at FROM loan.reservations WHERE reservation_id=$1",
            &[&backfill],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(due.to_rfc3339(), "2026-11-03T12:00:00+00:00");
    assert!(client
        .execute(
            "UPDATE loan.reservations SET due_at=NULL WHERE reservation_id=$1",
            &[&backfill]
        )
        .await
        .is_err());
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM loan.reservations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        3
    );
    println!("PASS existing-row migration: invalid dates abort without backfill, valid dates preserved, null dates backfilled, NOT NULL enforced, all original rows retained");
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run make backend-test-postgres"]
async fn postgres_tests_seed_preserves_live_inventory_and_reservations() {
    let pg = database("phase4_seed").await;
    pg.run_migrations(true).await.unwrap();
    let receipt = pg
        .reserve_device(
            student("seed-user"),
            "dell-latitude-7440".into(),
            2,
            "seed-rerun".into(),
        )
        .await
        .unwrap();
    let (client, _permit) = pg.connect_client().await.unwrap();
    client.batch_execute("UPDATE loan.device_models SET description='staff edit' WHERE id='dell-latitude-7440'; DELETE FROM loan.device_units WHERE unit_id='unit-ipad-001'; UPDATE loan.device_models SET total_units=0, available_units=0 WHERE id='ipad-air-m2'").await.unwrap();
    let snapshot = "SELECT jsonb_build_object('models',(SELECT jsonb_agg(to_jsonb(m) ORDER BY id) FROM loan.device_models m),'units',(SELECT jsonb_agg(to_jsonb(u) ORDER BY unit_id) FROM loan.device_units u),'reservations',(SELECT jsonb_agg(to_jsonb(r) ORDER BY reservation_id) FROM loan.reservations r))::text";
    let before: String = client.query_one(snapshot, &[]).await.unwrap().get(0);
    pg.run_migrations(true).await.unwrap();
    pg.run_migrations(true).await.unwrap();
    let after: String = client.query_one(snapshot, &[]).await.unwrap().get(0);
    assert_eq!(
        before, after,
        "seed must not reset edits, counts, units or loans"
    );
    assert_eq!(
        pg.list_reservations(student("seed-user")).await.unwrap()[0].reservation_id,
        receipt.reservation_id
    );
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run make backend-test-postgres"]
async fn postgres_tests_seed_collision_rolls_back_new_models() {
    let pg = database("phase4_seed_collision").await;
    pg.run_migrations(false).await.unwrap();
    seed(&pg).await;
    let (client, _permit) = pg.connect_client().await.unwrap();
    client
        .batch_execute("UPDATE loan.device_units SET unit_id='unit-lat-001' WHERE unit_id='unit'")
        .await
        .unwrap();
    assert!(pg.run_migrations(true).await.is_err());
    let row = client.query_one("SELECT (SELECT count(*) FROM loan.device_models), (SELECT model_id FROM loan.device_units WHERE unit_id='unit-lat-001')", &[]).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), 1);
    assert_eq!(row.get::<_, String>(1), "model");
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run make backend-test-postgres"]
async fn postgres_tests_migrations_run_without_database_create_privilege() {
    // Mirrors the Azure bootstrap (initialize-loan-roles.sql): the migrations role owns the
    // `loan` schema but has only CONNECT on the database, so it cannot run CREATE SCHEMA.
    let owner = database("phase4_least_privilege").await;
    let (client, _permit) = owner.connect_client().await.unwrap();
    client
        .batch_execute(
            "DO $$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'rc_migrator') THEN
               CREATE ROLE rc_migrator LOGIN PASSWORD 'local-phase1-test'; END IF; END $$;
             REVOKE ALL ON DATABASE phase4_least_privilege FROM PUBLIC;
             GRANT CONNECT ON DATABASE phase4_least_privilege TO rc_migrator;
             CREATE SCHEMA loan AUTHORIZATION rc_migrator;",
        )
        .await
        .unwrap();
    let migrator = PostgresAdapter::new(
        PostgresConfig {
            host: "localhost".into(),
            port: std::env::var("RC_TEST_PG_PORT").unwrap().parse().unwrap(),
            database: "phase4_least_privilege".into(),
            user: "rc_migrator".into(),
            static_password: Some("local-phase1-test".into()),
            max_connections: 2,
            publisher_role_name: None,
        },
        None,
    );
    migrator.run_migrations(true).await.unwrap();
    migrator.run_migrations(true).await.unwrap();
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run make backend-test-postgres"]
async fn postgres_tests_campus_catalogue_and_selected_periods() {
    let pg = database("campus_catalogue").await;
    pg.run_migrations(true).await.unwrap();
    let items = pg.list_catalogue().await.unwrap();
    assert_eq!(items.len(), 36);
    assert!(items.iter().all(|i| i.loan_duration_days == 14
        && i.image_url.is_some()
        && i.image_alt.is_some()
        && (3..=6).contains(&i.specifications.len())));
    let (client, _permit) = pg.connect_client().await.unwrap();
    let inconsistent: i64 = client.query_one("SELECT count(*) FROM loan.device_models m WHERE total_units <> (SELECT count(*) FROM loan.device_units u WHERE u.model_id=m.id) OR available_units <> (SELECT count(*) FROM loan.device_units u WHERE u.model_id=m.id AND u.status='AVAILABLE')", &[]).await.unwrap().get(0);
    assert_eq!(inconsistent, 0);
    assert_eq!(
        client
            .query_one(
                "SELECT count(*) FROM loan.device_units WHERE status IN ('RESERVED','COLLECTED')",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    for days in [0, -1, 15] {
        assert!(pg
            .reserve_device(
                student("bad"),
                "dell-latitude-7440".into(),
                days,
                "bad".into()
            )
            .await
            .is_err());
    }
    assert!(pg.list_reservations(staff()).await.unwrap().is_empty());
    assert_eq!(pg.list_catalogue().await.unwrap(), items);
    assert_eq!(
        client
            .query_one("SELECT count(*) FROM loan.outbox_events", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    for (model, days) in [("dell-latitude-7440", 1), ("macbook-air-m3", 14)] {
        let r = pg
            .reserve_device(student("borrower"), model.into(), days, "valid".into())
            .await
            .unwrap();
        assert_eq!(
            r.due_at - r.reserved_at,
            chrono::Duration::days(i64::from(days))
        );
    }
    let before = pg.list_reservations(staff()).await.unwrap();
    pg.run_migrations(true).await.unwrap();
    let after = pg.list_reservations(staff()).await.unwrap();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
    assert!(client
        .batch_execute(
            "UPDATE loan.device_models SET specifications='[1]' WHERE id='dell-latitude-7440'"
        )
        .await
        .is_err());
    assert!(client
        .batch_execute(
            "UPDATE loan.device_models SET loan_duration_days=15 WHERE id='dell-latitude-7440'"
        )
        .await
        .is_err());
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL; run make backend-test-postgres"]
async fn postgres_tests_campus_metadata_backfill_preserves_staff_edits() {
    let pg = database("campus_backfill").await;
    pg.run_migrations(false).await.unwrap();
    let (client, _permit) = pg.connect_client().await.unwrap();
    client.batch_execute("INSERT INTO loan.device_models (id,name,category,description,total_units,available_units) VALUES ('dell-latitude-7440','Dell Latitude 7440','Laptop','14-inch business laptop with 16 GB RAM and USB-C dock support for 2-day campus loans.',0,0), ('ipad-air-m2','Staff tablet','Tablet','Staff custom configuration',0,0), ('unrelated','Other equipment','Other','Staff item',0,0)").await.unwrap();
    pg.run_migrations(true).await.unwrap();
    let items = pg.list_catalogue().await.unwrap();
    let dell = items.iter().find(|i| i.id == "dell-latitude-7440").unwrap();
    assert_eq!(dell.total_units, 0);
    assert_eq!(dell.specifications.len(), 5);
    assert!(dell.description.contains("512 GB"));
    let ipad = items.iter().find(|i| i.id == "ipad-air-m2").unwrap();
    assert_eq!(ipad.description, "Staff custom configuration");
    assert!(ipad.specifications.is_empty());
    assert!(ipad.image_url.is_none());
    assert!(items.iter().any(|i| i.id == "unrelated"));
}
