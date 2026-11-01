\set ON_ERROR_STOP on
-- Run as the configured Entra administrator from a private-network runner.
-- Supply psql variables loan_name/loan_oid, publisher_name/publisher_oid,
-- migrations_name/migrations_oid from the platform handoff identities.
-- Object IDs are principal_id, never client_id. No passwords are used.
\connect postgres
SELECT format('SELECT * FROM pgaadauth_create_principal_with_oid(%L, %L, %L, false, false)', :'loan_name', :'loan_oid', 'service')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'loan_name') \gexec
SELECT format('SELECT * FROM pgaadauth_create_principal_with_oid(%L, %L, %L, false, false)', :'publisher_name', :'publisher_oid', 'service')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'publisher_name') \gexec
SELECT format('SELECT * FROM pgaadauth_create_principal_with_oid(%L, %L, %L, false, false)', :'migrations_name', :'migrations_oid', 'service')
WHERE NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'migrations_name') \gexec
\connect loan
BEGIN;
REVOKE ALL ON DATABASE loan FROM PUBLIC;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
GRANT CONNECT ON DATABASE loan TO :"loan_name", :"publisher_name", :"migrations_name";
CREATE SCHEMA IF NOT EXISTS loan AUTHORIZATION :"migrations_name";
ALTER SCHEMA loan OWNER TO :"migrations_name";
GRANT USAGE ON SCHEMA loan TO :"loan_name", :"publisher_name";
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA loan TO :"loan_name";
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA loan TO :"loan_name";
ALTER DEFAULT PRIVILEGES FOR ROLE :"migrations_name" IN SCHEMA loan
  GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO :"loan_name";
ALTER DEFAULT PRIVILEGES FOR ROLE :"migrations_name" IN SCHEMA loan
  GRANT USAGE, SELECT ON SEQUENCES TO :"loan_name";
-- Phase 2 migration grants publisher SELECT/UPDATE only on its outbox table(s).
-- No default table grant for publisher; no Notifications PostgreSQL principal.
COMMIT;
