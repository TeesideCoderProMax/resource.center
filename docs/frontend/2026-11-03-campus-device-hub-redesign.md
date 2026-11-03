# Campus Device Hub redesign (2026-11-03)

Status: **implemented and checked locally** on `feat/frontend-stitch-catalogue`. Not deployed.

The source was the user's four Stitch screens (catalogue, device detail, cart, admin panel) and a
"Campus Device Hub" PRD. The user chose to match the screens **using the real Loan API only**
and to add a **browser-side loan bag**. No backend, contract or ADR changes.

## What exists

| Route | Screen | Data and actions |
| --- | --- | --- |
| `/` | Catalogue: category chips, filters (search, available-only, loan length), sort, grid/list | `GET catalogue`; reserve, subscribe, add to bag. Header search sends `?q=`; ⌘/Ctrl+K focuses it |
| `/devices/[id]` | Detail: status, facts, loan rules, reservation panel with a loan-window calendar | Same catalogue item; reserve/subscribe/add to bag. The calendar only shows the window; the API sets the due time |
| `/bag` | Checkout & Reservation Review: items, borrower, summary, two required acknowledgements | Bag held in `localStorage` (`lib/bag.tsx`). "Reserve all" sends one `POST reservations` per model and reports each result (partial success is possible); 409 offers subscribe |
| `/staff` | Staff Admin: section sidebar, telemetry cards, collection queue, loans table with collect/return confirm, inventory table, CSV export | `GET reservations` (newest 200) and the catalogue. Telemetry is computed by `deskStats`. CSV export runs in the browser |
| `/reservations` | My Loans / All Loans | Unchanged logic, restyled |

Nav labels are now Catalog / My Loans (staff: All Loans) / Staff Admin.

## PRD items deliberately not built

These are unsupported by the Loan API or conflict with the accepted design (instant reservation
with a desk hand-off):
- smart lockers, PIN/SMS dispatch
- approve/deny queue, project-purpose binding
- date-range booking, add-on peripherals
- fees and deposits
- department/brand facets
- product photos
- inventory editing, CSV import, maintenance states
- academic standing and student IDs
- Shibboleth/SAML

Any of these needs new Loan API work and an ADR first.

## Checks run (local, 2026-11-03)

In `frontend/`:
- `npm run typecheck` passed.
- `npm test` passed 36/36, including new `LoanBag.test.tsx`, catalogue filter tests and staff telemetry/inventory tests.
- `npm run build` passed.
- `npx playwright test` passed 13/13 against the fake Loan API, including new bag and staff-admin flows.

Signed-in screens were reviewed from Playwright screenshots at 1440px and 390px.
