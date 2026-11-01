use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const STANDARD_LOAN_DURATION_DAYS: i64 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceModelSummary {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub loan_duration_days: i32,
    pub total_units: i32,
    pub available_units: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Student,
    Staff,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedActor {
    pub subject: String,
    pub email: String,
    pub roles: Vec<Role>,
}

impl AuthenticatedActor {
    pub fn has_role(&self, expected: &Role) -> bool {
        self.roles.iter().any(|r| r == expected)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("actor lacks required role: {0}")]
    ForbiddenRole(&'static str),
    #[error("no available units for device model {0}")]
    NoAvailableUnits(String),
    #[error(
        "device model {0} currently has available units; reserve directly instead of subscribing"
    )]
    UnitsCurrentlyAvailable(String),
    #[error("invalid loan state transition from {from} to {to}")]
    InvalidTransition { from: String, to: String },
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
}

/// Computes the two-day return due timestamp from the reservation timestamp.
pub fn compute_two_day_due_at(reserved_at: DateTime<Utc>) -> DateTime<Utc> {
    reserved_at + Duration::days(STANDARD_LOAN_DURATION_DAYS)
}

/// Validates that an actor is authorized to reserve a device or subscribe to alerts.
pub fn ensure_student_or_staff(actor: &AuthenticatedActor) -> Result<(), DomainError> {
    if actor.has_role(&Role::Student) || actor.has_role(&Role::Staff) {
        Ok(())
    } else {
        Err(DomainError::ForbiddenRole("student"))
    }
}

/// Validates that an actor is authorized to record collections or returns.
pub fn ensure_staff(actor: &AuthenticatedActor) -> Result<(), DomainError> {
    if actor.has_role(&Role::Staff) {
        Ok(())
    } else {
        Err(DomainError::ForbiddenRole("staff"))
    }
}

/// Generates a deterministic, single-recipient v5 UUID event ID from a business transition ID
/// and recipient so repeated outbox creation or replay maps to the same `(eventId, id)` pair.
pub fn deterministic_event_id(transition_key: &str, recipient: &str) -> String {
    let input = format!("{transition_key}:{recipient}");
    format!(
        "evt-{}",
        Uuid::new_v5(&Uuid::NAMESPACE_OID, input.as_bytes())
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_day_loan_due_calculation() {
        let now = DateTime::parse_from_rfc3339("2026-10-31T10:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let due = compute_two_day_due_at(now);
        assert_eq!(due.to_rfc3339(), "2026-11-02T10:00:00+00:00");
    }

    #[test]
    fn role_enforcement_checks_student_and_staff() {
        let student = AuthenticatedActor {
            subject: "auth0|student1".to_string(),
            email: "s@tees.ac.uk".to_string(),
            roles: vec![Role::Student],
        };
        assert!(ensure_student_or_staff(&student).is_ok());
        assert_eq!(
            ensure_staff(&student),
            Err(DomainError::ForbiddenRole("staff"))
        );
    }

    #[test]
    fn deterministic_event_id_is_stable() {
        let a = deterministic_event_id("return:res-1", "student@tees.ac.uk");
        let b = deterministic_event_id("return:res-1", "student@tees.ac.uk");
        let c = deterministic_event_id("return:res-1", "other@tees.ac.uk");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}

/// Snapshot loaded under the model lock. These values never come from HTTP callers.
#[derive(Debug, Clone)]
pub struct LockedModel {
    pub id: String,
    pub name: String,
    pub available_units: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reservation {
    pub reservation_id: Uuid,
    pub model_id: String,
    pub unit_id: String,
    pub student_sub: String,
    pub student_email: String,
    pub status: String,
    pub reserved_at: DateTime<Utc>,
    pub due_at: DateTime<Utc>,
    pub collected_at: Option<DateTime<Utc>>,
    pub returned_at: Option<DateTime<Utc>>,
    pub correlation_id: String,
}

pub fn ensure_can_reserve(model: &LockedModel) -> Result<(), DomainError> {
    if model.available_units <= 0 {
        return Err(DomainError::NoAvailableUnits(model.id.clone()));
    }
    Ok(())
}

pub fn ensure_can_subscribe(model: &LockedModel) -> Result<(), DomainError> {
    if model.available_units > 0 {
        return Err(DomainError::UnitsCurrentlyAvailable(model.id.clone()));
    }
    Ok(())
}

impl Reservation {
    pub fn collect(&mut self, now: DateTime<Utc>) -> Result<(), DomainError> {
        self.transition("RESERVED", "COLLECTED")?;
        self.collected_at = Some(now);
        Ok(())
    }

    pub fn return_device(&mut self, now: DateTime<Utc>) -> Result<(), DomainError> {
        self.transition("COLLECTED", "RETURNED")?;
        self.returned_at = Some(now);
        Ok(())
    }

    fn transition(&mut self, from: &str, to: &str) -> Result<(), DomainError> {
        if self.status != from {
            return Err(DomainError::InvalidTransition {
                from: self.status.clone(),
                to: to.into(),
            });
        }
        self.status = to.into();
        Ok(())
    }
}
