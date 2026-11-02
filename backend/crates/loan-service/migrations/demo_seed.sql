-- Deterministic, idempotent Test demo seed data for Campus Device Loan.
-- Separated from routine schema migrations; run only when SEED_DEMO_DATA=true.

WITH new_models AS (
INSERT INTO loan.device_models (id, name, category, description, loan_duration_days, total_units, available_units, updated_at)
VALUES
    ('dell-latitude-7440', 'Dell Latitude 7440', 'Laptop', '14-inch business laptop with 16 GB RAM and USB-C dock support for 2-day campus loans.', 2, 2, 2, NOW()),
    ('macbook-air-m3', 'Apple MacBook Air 13" (M3)', 'Laptop', '13-inch macOS laptop for mobile development and media coursework (2-day loan).', 2, 1, 1, NOW()),
    ('ipad-air-m2', 'iPad Air 11" + Apple Pencil', 'Tablet', 'Tablet kit for design and annotation studio sessions (2-day loan).', 2, 1, 1, NOW()),
    ('sony-alpha-a6400', 'Sony Alpha a6400 Kit', 'Camera', 'Mirrorless camera with 16-50mm lens and SD card for media production (2-day loan).', 2, 1, 0, NOW())
ON CONFLICT (id) DO NOTHING
RETURNING id
)

INSERT INTO loan.device_units (unit_id, model_id, serial_number, status, updated_at)
SELECT unit_id, model_id, serial_number, status, NOW() FROM (VALUES
    ('unit-lat-001', 'dell-latitude-7440', 'SN-DELL-7440-001', 'AVAILABLE'),
    ('unit-lat-002', 'dell-latitude-7440', 'SN-DELL-7440-002', 'AVAILABLE'),
    ('unit-mba-001', 'macbook-air-m3', 'SN-APPLE-MBA-001', 'AVAILABLE'),
    ('unit-ipad-001', 'ipad-air-m2', 'SN-APPLE-IPAD-001', 'AVAILABLE'),
    ('unit-sony-001', 'sony-alpha-a6400', 'SN-SONY-A6400-001', 'COLLECTED')
) AS units(unit_id, model_id, serial_number, status)
WHERE model_id IN (SELECT id FROM new_models);
-- A unit/serial collision aborts the transaction; never create inconsistent counts.
