# Catalogue and loan periods — 2026-11-04

Implemented: 36 seeded campus models with matching descriptions and 3–6 ordered
specs each. Each model has a local WebP image and an attribution/rights record.
The card, detail page and bag use the same image. Device facts show specs; loan
details show stock, collection and limits. See `docs/catalogue/README.md`.

Borrowers choose 1–14 whole days for each reservation, starting at 1. The BFF
forwards the chosen period; the API validates it under the model lock and saves
the resulting due date. Existing reservations keep their due dates. Migration
0003 adds image/spec fields and changes model limits. Deploy API and web together;
old clients omit the required period and old APIs still calculate two days.

## Local checks

- Rust workspace tests: 29 passed; six PostgreSQL tests run separately; the real
  Cosmos test remains opt-in and outside this change. Clippy and format passed.
- Real PostgreSQL: six tests passed, covering fresh/old migrations, metadata
  backfill, edited descriptions, seed reruns, least-privilege migration roles,
  collisions, stock counts, concurrency, rollback and saved due dates.
- Frontend: 40 component/unit tests passed. Production build and typecheck passed.
- Browser: 15 tests passed, including 14-day due dates, calendar updates, loan bag,
  identity protection and phone width. Desktop/phone screenshots checked.
- Release checks: 18 tests passed. Production dependency audit found no issues.
  Terraform format and all three Test roots validated.
- Runtime containers: migration and repeat seed passed; all 36 catalogue records
  and images passed HTTP checks. A 40-request local probe had zero errors.
- Local app loaded: http://localhost:3100 (Loan API on localhost:8081).
  This uses the existing disposable `dev.sh` stack; stopping it removes its database.
- Azure Test: GitHub checks, runtime image checks, migration/seed and deployed
  smoke passed. A separate public HTTP check found 36 models, 80 physical units,
  a maximum of 14 days for every model, and at least three specs per model. All
  36 image URLs returned valid WebP files. Web health and readiness returned 200
  with the intended version and a reachable Loan API.

## Image records

`frontend/public/catalogue/sources.json` records sources, authors and licences or
rights status. `/image-credits` shows them in the app. Manufacturer/retailer images
without an open licence are marked as such. The image-generation service returned
404; no generated image or CLI fallback was used.

## Release

- Code: `4151f56`; calendar-check fix and staff maximum label: `b7062ee`.
- First Test run `41853739209` failed verification before deployment. The new
  calendar test matched a hidden streamed copy. The test now selects visible
  summaries. All 15 browser tests and 40 frontend tests passed again.
- Successful Test run: [41855299241](https://github.com/TeesideCoderProMax/resource.center/actions/runs/41855299241).
- Version: `b7062ee1ae72ad9cbbf61a67d2701189e09466ed`.
- Migration execution `rc-test-0330980f-migrations-84xkjti` succeeded.
- Exact image digests and release identity: `docs/catalogue/2026-11-04-test-release.json`.
- Hosted app: https://rc-test-0330980f-web.livelywave-2cc0564d.francecentral.azurecontainerapps.io

No Staging or Production release occurred. Signed browser tests used the fake API;
this change does not claim a new live Auth0 login or email-delivery test. Public
Azure checks ran from the edge tool because the local host's HTTPS reads to the
Container Apps domains timed out. Azure CLI and GitHub release checks worked.

Test ports can be overridden with `E2E_WEB_PORT` and `E2E_API_PORT` to leave the
local app running. The local app remains on port 3100 with the 36 seeded models;
the separate API uses port 8081.
