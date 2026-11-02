# Phase 3 — Secure student and staff flows

Status: **implemented locally; live acceptance blocked** — see [phase-3-handoff.md](phase-3-handoff.md). One isolated implementation chat after phases 1–2. Follow `docs/backend/README.md`.

## Goal and entry context

Make the backend usable through a small complete web UI and prove authentication, role controls and notification outcomes. This includes the frontend work needed to validate the backend, not a visual redesign.

Read ADR 0004, phase-1/2 handoffs, `contracts.md` and `notification-recovery.md`. Serena: `assessment/cloud_module_criteria`, `architecture/service_boundaries`, `architecture/testing`, `tech_stack`. Inspect current Auth0 JWT code, frontend and environment templates; discover existing tenant/provider settings before asking for missing values. Do not expose credentials.

## Work

1. Implement Auth0 OIDC Authorization Code + PKCE sign-in/logout with state/nonce checks through a supported integration. Configure audience, scopes, callback/logout URLs and role claims per environment. Choose a clear token/session storage design and document its CSRF/XSS consequences; keep privileged secrets server-side.
2. Finish JWT validation and safe JWKS refresh/error handling. Use Auth0 subject as identity, trusted role claims for authorization and trusted recipient data. A user-supplied email or role must not override the authenticated actor. Fail closed without reporting a configuration problem as successful auth.
3. Implement required user/staff query endpoints from phase 1 if still outstanding. Enforce ownership in the backend and minimize exposed personal data. Support staff finding the reservation to collect/return and students seeing their reservation and return date; prevent access through guessed IDs.
4. Build minimal catalogue/sign-in/reserve/subscribe/my-reservations and staff collection/return flows. Show useful error/loading/success states and the reservation-time due date. Treat backend state as authoritative; handle last-unit conflicts without a false success. Show brand/model/category clearly.
5. Wire Resend mode using existing secret/configuration paths and explicitly authorized test recipients. Verify collection, return and availability emails, including multiple eligible subscribers. Record sender identity, redacted outcome/correlation and received-email evidence without secrets. Preserve fake mode for local/CI tests.
6. Run the complete workflow against real authenticated identities where access exists. Keep fake tests and live proof separate. If credentials or recipient authorization are unavailable, complete local work and record the exact remaining check; do not claim complete live acceptance.
7. Update API/diagram/evidence documentation to the actual routes, UI and security flow. Record tutor-approved MVP/fake scope if supplied; never infer it from an agent-created diagram.

## Completion checks

- JWT tests cover missing token, bad signature, wrong issuer/audience, expiry, role claims and JWKS failures/refresh. Valid student credentials get 403 on staff actions; staff can perform approved actions. Test cross-user data access rejection.
- Browser/API tests prove public catalogue, sign-in, availability, reserve with due date, subscribe, staff collection/return and clear 409/validation failures as defined by the contract. Browser business success must correspond to persisted state.
- Live Test evidence, when authorized/configured, shows valid Auth0 sign-in, real HTTP 403 and all three email cases. Fake success is labelled fake.
- Run relevant Rust checks, frontend typecheck/build/audit and the new documented flow tests. Review CORS, TLS, logs and client bundle configuration for secret leakage.

## Scope and handoff

Do not add new product features or redesign hosting. Save `docs/backend/phase-3-handoff.md` with API/UI contracts, setup instructions, names of needed secrets, verified fake/live modes, exact commands and redacted evidence. Phase 4 needs runnable smoke/end-to-end checks and an accurate list of remaining external blockers.

## Starting prompt

```text
Implement phase 3 of the Resource Center backend plan in /home/cabozzz/dev/resource.center. Read AGENTS.md, activate this Serena project, then read backend/plan, docs/backend/README.md, docs/backend/phase-3-secure-flows.md, ADR 0004 and phase-1/2 handoffs. Implement the Auth0/RBAC integration and minimal student/staff web flows required to exercise the backend. Preserve backend ownership checks and fake test modes. Discover existing configuration; use only authorized email recipients and report missing live inputs. Complete the phase-3 handoff, update current memories and AI_LOG.md. Do not start phase 4.
```
