use std::collections::HashSet;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_LOAN_DURATION_DAYS: i32 = 14;
pub const MAX_IMAGE_DATA_BYTES: usize = 700_000;

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
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_alt: Option<String>,
    #[serde(default)]
    pub specifications: Vec<DeviceSpecification>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSpecification {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceUnitSummary {
    pub unit_id: String,
    pub serial_number: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaffInventoryModel {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub loan_duration_days: i32,
    pub total_units: i32,
    pub available_units: i32,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_alt: Option<String>,
    #[serde(default)]
    pub specifications: Vec<DeviceSpecification>,
    pub units: Vec<DeviceUnitSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDeviceUnitInput {
    pub serial_number: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDeviceModelInput {
    pub id: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub loan_duration_days: i32,
    pub image_url: String,
    pub image_alt: String,
    pub specifications: Vec<DeviceSpecification>,
    pub units: Vec<NewDeviceUnitInput>,
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
    #[error("resource already exists: {0}")]
    AlreadyExists(String),
    #[error("resource in use: {0}")]
    ResourceInUse(String),
}

/// Validates a model slug ID (`[a-z0-9-]`, 2..=64 chars, no leading/trailing/double hyphens).
pub fn validate_model_id(raw: &str) -> Result<String, DomainError> {
    let trimmed = raw.trim();
    if trimmed.len() < 2
        || trimmed.len() > 64
        || trimmed.contains("--")
        || !trimmed
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || !trimmed.as_bytes()[0].is_ascii_alphanumeric()
        || !trimmed.as_bytes()[trimmed.len() - 1].is_ascii_alphanumeric()
    {
        return Err(DomainError::InvalidInput(
            "id must be 2–64 lowercase letters, digits, or single hyphens",
        ));
    }
    Ok(trimmed.to_string())
}

/// Validates a mandatory product image URL: `/catalogue/...`, `https://...`, or base64 webp/png/jpeg data URI.
pub fn validate_image_url(raw: &str) -> Result<String, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_IMAGE_DATA_BYTES {
        return Err(DomainError::InvalidInput(
            "imageUrl is required and must be at most 700 KB",
        ));
    }
    if trimmed.starts_with("/catalogue/") {
        if trimmed.len() == "/catalogue/".len()
            || trimmed.contains("..")
            || trimmed.contains('\\')
            || trimmed.chars().any(char::is_whitespace)
        {
            return Err(DomainError::InvalidInput(
                "imageUrl relative path must point to a valid /catalogue/ asset",
            ));
        }
        return Ok(trimmed.to_string());
    }
    if trimmed.starts_with("https://") {
        if trimmed.len() == "https://".len()
            || trimmed.len() > 2048
            || trimmed.chars().any(char::is_whitespace)
        {
            return Err(DomainError::InvalidInput(
                "imageUrl https:// URL must be a valid URL up to 2048 characters",
            ));
        }
        return Ok(trimmed.to_string());
    }
    for prefix in [
        "data:image/webp;base64,",
        "data:image/png;base64,",
        "data:image/jpeg;base64,",
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if !rest.is_empty()
                && rest
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
            {
                return Ok(trimmed.to_string());
            }
            return Err(DomainError::InvalidInput(
                "imageUrl data URI must contain valid base64 image data",
            ));
        }
    }
    Err(DomainError::InvalidInput(
        "imageUrl must be a /catalogue/ path, https:// URL, or base64 webp/png/jpeg image",
    ))
}

/// Validates a staff-editable unit status (`AVAILABLE` or `MAINTENANCE`).
pub fn validate_unit_status_target(raw: &str) -> Result<String, DomainError> {
    let status = raw.trim().to_ascii_uppercase();
    match status.as_str() {
        "AVAILABLE" | "MAINTENANCE" => Ok(status),
        _ => Err(DomainError::InvalidInput(
            "status must be AVAILABLE or MAINTENANCE",
        )),
    }
}

/// Validates a new physical unit input (mandatory serial number and valid initial status).
pub fn validate_new_unit_input(
    input: NewDeviceUnitInput,
) -> Result<NewDeviceUnitInput, DomainError> {
    let serial_number = input.serial_number.trim().to_string();
    if serial_number.is_empty() || serial_number.len() > 120 {
        return Err(DomainError::InvalidInput(
            "unit serialNumber is required and must be at most 120 characters",
        ));
    }
    let status = validate_unit_status_target(&input.status)?;
    Ok(NewDeviceUnitInput {
        serial_number,
        status,
    })
}

fn require_text(raw: &str, max_len: usize, err: &'static str) -> Result<String, DomainError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().count() > max_len {
        return Err(DomainError::InvalidInput(err));
    }
    Ok(trimmed.to_string())
}

/// Validates that every product field is provided when staff adds a new device model.
pub fn validate_create_model_input(
    input: CreateDeviceModelInput,
) -> Result<CreateDeviceModelInput, DomainError> {
    let id = validate_model_id(&input.id)?;
    let name = require_text(
        &input.name,
        160,
        "name is required and must be at most 160 characters",
    )?;
    let category = require_text(
        &input.category,
        80,
        "category is required and must be at most 80 characters",
    )?;
    let description = require_text(
        &input.description,
        1000,
        "description is required and must be at most 1000 characters",
    )?;
    if !(1..=MAX_LOAN_DURATION_DAYS).contains(&input.loan_duration_days) {
        return Err(DomainError::InvalidInput(
            "loanDurationDays must be a whole number between 1 and 14",
        ));
    }
    let image_url = validate_image_url(&input.image_url)?;
    let image_alt = require_text(
        &input.image_alt,
        200,
        "imageAlt is required and must be at most 200 characters",
    )?;
    if input.specifications.is_empty() || input.specifications.len() > 30 {
        return Err(DomainError::InvalidInput(
            "at least 1 specification (up to 30) is required",
        ));
    }
    let mut specifications = Vec::with_capacity(input.specifications.len());
    for spec in input.specifications {
        specifications.push(DeviceSpecification {
            label: require_text(
                &spec.label,
                120,
                "each specification label is required (max 120 characters)",
            )?,
            value: require_text(
                &spec.value,
                240,
                "each specification value is required (max 240 characters)",
            )?,
        });
    }
    if input.units.is_empty() || input.units.len() > 50 {
        return Err(DomainError::InvalidInput(
            "at least 1 initial unit (up to 50) is required",
        ));
    }
    let mut units = Vec::with_capacity(input.units.len());
    let mut seen_serials = HashSet::with_capacity(input.units.len());
    for unit in input.units {
        let validated = validate_new_unit_input(unit)?;
        if !seen_serials.insert(validated.serial_number.to_ascii_lowercase()) {
            return Err(DomainError::InvalidInput(
                "unit serial numbers must be unique within the request",
            ));
        }
        units.push(validated);
    }

    Ok(CreateDeviceModelInput {
        id,
        name,
        category,
        description,
        loan_duration_days: input.loan_duration_days,
        image_url,
        image_alt,
        specifications,
        units,
    })
}

/// Computes the return timestamp after validating the chosen whole-day period.
pub fn compute_due_at(
    reserved_at: DateTime<Utc>,
    days: i32,
    maximum: i32,
) -> Result<DateTime<Utc>, DomainError> {
    if !(1..=MAX_LOAN_DURATION_DAYS).contains(&maximum) || !(1..=maximum).contains(&days) {
        return Err(DomainError::InvalidInput(
            "loanDurationDays must be a whole number from 1 to the model maximum (at most 14)",
        ));
    }
    Ok(reserved_at + Duration::days(i64::from(days)))
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
        let due = compute_due_at(now, 2, 14).unwrap();
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

    #[test]
    fn model_and_image_validation_enforces_mandatory_fields() {
        assert_eq!(validate_model_id(" sony-a7-iv ").unwrap(), "sony-a7-iv");
        assert!(validate_model_id("Bad Slug").is_err());
        assert!(validate_model_id("-bad").is_err());
        assert!(validate_model_id("bad--slug").is_err());

        assert!(validate_image_url("/catalogue/dell-latitude-7440.webp").is_ok());
        assert!(validate_image_url("https://example.edu/photo.webp").is_ok());
        assert!(validate_image_url("data:image/webp;base64,UklGRh4AAABXRUJQVlA4").is_ok());
        assert!(validate_image_url("data:image/png;base64,iVBORw0KGgo=").is_ok());
        assert!(validate_image_url("data:image/jpeg;base64,/9j/4AAQ").is_ok());
        assert!(validate_image_url("").is_err());
        assert!(validate_image_url("javascript:alert(1)").is_err());
        assert!(validate_image_url("data:image/svg+xml;base64,PHN2Zz4=").is_err());
        assert!(validate_image_url("/catalogue/../secret.webp").is_err());

        assert_eq!(
            validate_unit_status_target("maintenance").unwrap(),
            "MAINTENANCE"
        );
        assert!(validate_unit_status_target("COLLECTED").is_err());

        let valid = CreateDeviceModelInput {
            id: "sony-fx3".into(),
            name: "Sony FX3".into(),
            category: "Camera".into(),
            description: "Cinema camera".into(),
            loan_duration_days: 7,
            image_url: "data:image/webp;base64,UklGRg==".into(),
            image_alt: "Sony FX3".into(),
            specifications: vec![DeviceSpecification {
                label: "Sensor".into(),
                value: "Full-frame".into(),
            }],
            units: vec![NewDeviceUnitInput {
                serial_number: "SN-FX3-001".into(),
                status: "available".into(),
            }],
        };
        assert!(validate_create_model_input(valid.clone()).is_ok());
        assert!(validate_create_model_input(CreateDeviceModelInput {
            specifications: vec![],
            ..valid.clone()
        })
        .is_err());
        assert!(validate_create_model_input(CreateDeviceModelInput {
            units: vec![],
            ..valid.clone()
        })
        .is_err());
        assert!(validate_create_model_input(CreateDeviceModelInput {
            units: vec![
                NewDeviceUnitInput {
                    serial_number: "SN-1".into(),
                    status: "AVAILABLE".into(),
                },
                NewDeviceUnitInput {
                    serial_number: "sn-1".into(),
                    status: "MAINTENANCE".into(),
                },
            ],
            ..valid
        })
        .is_err());
    }
}

/// Snapshot loaded under the model lock. These values never come from HTTP callers.
#[derive(Debug, Clone)]
pub struct LockedModel {
    pub id: String,
    pub name: String,
    pub available_units: i32,
    pub loan_duration_days: i32,
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
