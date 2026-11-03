# Stage 4 demo script (web)

Status: **script only, 2026-11-03**. No clip below has been recorded. Live steps need
the Test web URL, a configured Auth0 tenant with one student and one staff test user,
and (for email) approved recipients with live Resend delivery. Media rules come from
the [submission checklist](../assessment/submission-checklist.md): separate 20–30 s
H.264 MP4 clips, under five minutes in total, no visible secrets, tokens, real student
emails or unrelated tabs.

Set `WEB=https://<test web FQDN>`. Use a fresh private window per role.

| # | Media item | Route and action | What the viewer must see | Prerequisite |
| --- | --- | --- | --- | --- |
| 1 | Public availability | Signed out, open `$WEB/` | Catalogue with available/total counts and 2-day loan duration; "Sign in" in the header | Deployed web + API |
| 2 | Student sign-in (OIDC + PKCE) | Click **Sign in**, log in as the student test user | Auth0 Universal Login, redirect back, header shows the student role | Auth0 tenant + student user |
| 3 | Reserve | `$WEB/devices/<modelId>` → **Reserve** | Success notice with unit, due date (+2 days) and desk next step; count drops by one after reload | Item 2 |
| 4 | Last unit → subscribe | Reserve until 0 available, then reserve again | 409 notice offering **Subscribe**; subscribe confirmation "You will be emailed when … is available" | Item 2; model with few units |
| 5 | Student 403 | Navigate to `$WEB/staff` | "403 Forbidden." notice; no staff actions | Item 2 |
| 6 | Staff collect | Sign in as staff, `$WEB/staff` → filter *reserved* → **Mark collected** → **Confirm** | Two-step confirm, success notice with `collectedAt`, row moves to *collected* | Staff test user |
| 7 | Staff return | Same desk → **Mark returned** → **Confirm** | `returnedAt` shown; catalogue count rises | Item 6 |
| 8 | Availability email | Inbox of the subscribed approved recipient after item 7 | Availability email for the model | Live email approval and setup (backend) |
| 9 | Correlation trace | DevTools → Network on any `/api/loan/*` call → copy `x-correlation-id`; search it in Log Analytics | Same ID in web `loan_proxy` log, Loan API log and (for events) publisher/worker logs | Log Analytics access |
| 10 | Health and readiness | Open `$WEB/api/health` and `$WEB/api/ready` | `status: ok`; `status: ready`, `loanApi: reachable`, version = release SHA | Deployed release |
| 11 | CI/CD | GitHub Actions: CI run (frontend E2E job + `playwright-report` artifact), Deploy run on `qa` (images, migration, smoke, `release.json`) | Green checks, digests, smoke passed | Push to `qa` with deploy authorization |
| 12 | Scaling | Run the load probe below while showing Container Apps replica count | p50/p95, zero errors, replica count before/during/after | Item 10; approval for the probe |

Load probe (bounded, read-only):

```bash
python3 infra/scripts/load-check.py "$WEB/api/loan/catalogue" --requests 100 --concurrency 10
```

The web tier keeps no server-side session store (sessions are encrypted cookies), so
any replica can serve any request. Record results separately from local evidence.
