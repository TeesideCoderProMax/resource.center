-- Add catalogue metadata and allow borrower-selected whole-day periods.
-- Recorded reservation due dates remain unchanged.
ALTER TABLE loan.device_models DROP CONSTRAINT device_models_loan_duration_days_check;
ALTER TABLE loan.device_models ALTER COLUMN loan_duration_days SET DEFAULT 14;
UPDATE loan.device_models SET loan_duration_days = 14;
ALTER TABLE loan.device_models ADD CONSTRAINT device_models_loan_duration_days_check
    CHECK (loan_duration_days BETWEEN 1 AND 14);
ALTER TABLE loan.device_models ADD COLUMN image_url TEXT;
ALTER TABLE loan.device_models ADD COLUMN image_alt TEXT;
ALTER TABLE loan.device_models ADD COLUMN specifications JSONB NOT NULL DEFAULT '[]';
CREATE FUNCTION loan.valid_specifications(value JSONB) RETURNS BOOLEAN
LANGUAGE SQL IMMUTABLE STRICT AS $$
    SELECT CASE WHEN jsonb_typeof(value) <> 'array' THEN false ELSE
        NOT EXISTS (SELECT 1 FROM jsonb_array_elements(value) AS entry
            WHERE jsonb_typeof(entry) <> 'object'
               OR jsonb_typeof(entry->'label') IS DISTINCT FROM 'string'
               OR jsonb_typeof(entry->'value') IS DISTINCT FROM 'string'
               OR length(trim(entry->>'label')) = 0 OR length(trim(entry->>'value')) = 0)
        END;
$$;
ALTER TABLE loan.device_models ADD CONSTRAINT device_specifications_valid
    CHECK (loan.valid_specifications(specifications));
