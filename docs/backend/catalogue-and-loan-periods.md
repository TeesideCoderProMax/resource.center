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
- Azure Test checks: pending at this point.

## Image records

`frontend/public/catalogue/sources.json` records sources, authors and licences or
rights status. `/image-credits` shows them in the app. Manufacturer/retailer images
without an open licence are marked as such. The image-generation service returned
404; no generated image or CLI fallback was used.

## Release

Azure Test release is pending. No Staging or Production release is in scope.
