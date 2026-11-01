-- Migration 0001: Initial Loan Service schema and transactional outbox
-- Executed by the migrations managed identity (owner of schema `loan`).
-- Grants least-privilege SELECT/UPDATE on `loan.outbox_events` to the publisher principal.

CREATE TABLE IF NOT EXISTS loan.schema_migrations (
    version TEXT PRIMARY KEY,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS loan.device_models (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    description TEXT NOT NULL,
    loan_duration_days INTEGER NOT NULL DEFAULT 2 CHECK (loan_duration_days = 2),
    total_units INTEGER NOT NULL CHECK (total_units >= 0),
    available_units INTEGER NOT NULL CHECK (available_units >= 0 AND available_units <= total_units),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS loan.device_units (
    unit_id TEXT PRIMARY KEY,
    model_id TEXT NOT NULL REFERENCES loan.device_models(id),
    serial_number TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('AVAILABLE', 'RESERVED', 'COLLECTED', 'MAINTENANCE')),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS loan.reservations (
    reservation_id UUID PRIMARY KEY,
    model_id TEXT NOT NULL REFERENCES loan.device_models(id),
    unit_id TEXT NOT NULL REFERENCES loan.device_units(unit_id),
    student_sub TEXT NOT NULL,
    student_email TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('RESERVED', 'COLLECTED', 'RETURNED', 'CANCELLED')),
    reserved_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    collected_at TIMESTAMPTZ,
    due_at TIMESTAMPTZ,
    returned_at TIMESTAMPTZ,
    correlation_id TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS loan.availability_subscriptions (
    subscription_id UUID PRIMARY KEY,
    model_id TEXT NOT NULL REFERENCES loan.device_models(id),
    student_sub TEXT NOT NULL,
    student_email TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'NOTIFIED', 'CANCELLED')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    notified_at TIMESTAMPTZ,
    UNIQUE (model_id, student_sub, status)
);

CREATE TABLE IF NOT EXISTS loan.outbox_events (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    contract_version INTEGER NOT NULL DEFAULT 1,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    correlation_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING' CHECK (status IN ('PENDING', 'PUBLISHED', 'FAILED')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    claimed_until TIMESTAMPTZ,
    published_at TIMESTAMPTZ,
    last_error TEXT
);

CREATE INDEX IF NOT EXISTS idx_outbox_events_pending
    ON loan.outbox_events (occurred_at ASC)
    WHERE status = 'PENDING';
