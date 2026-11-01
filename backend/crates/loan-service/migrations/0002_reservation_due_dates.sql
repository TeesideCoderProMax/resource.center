-- Preserve recorded dates. Abort before changing data if any recorded date is invalid.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM loan.reservations WHERE NOT isfinite(reserved_at)
        OR (due_at IS NOT NULL AND (NOT isfinite(due_at) OR due_at < reserved_at))) THEN
        RAISE EXCEPTION 'Invalid reservation dates: inspect reservation_id, reserved_at, due_at before migration 0002';
    END IF;
END $$;
UPDATE loan.reservations SET due_at = reserved_at + INTERVAL '48 hours' WHERE due_at IS NULL;
ALTER TABLE loan.reservations ALTER COLUMN due_at SET NOT NULL;
ALTER TABLE loan.reservations ADD CONSTRAINT reservation_due_valid
    CHECK (isfinite(reserved_at) AND isfinite(due_at) AND due_at >= reserved_at) NOT VALID;
ALTER TABLE loan.reservations VALIDATE CONSTRAINT reservation_due_valid;
-- Only ACTIVE rows must be unique. A student can subscribe again after notification.
-- The old all-status uniqueness prevents a second NOTIFIED row on the next return.
ALTER TABLE loan.availability_subscriptions
    DROP CONSTRAINT availability_subscriptions_model_id_student_sub_status_key;
CREATE UNIQUE INDEX one_active_subscription ON loan.availability_subscriptions (model_id, student_sub)
    WHERE status = 'ACTIVE';
