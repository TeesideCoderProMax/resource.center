# Campus catalogue

The catalogue has 36 seeded models. `backend/catalogue/campus-items.json` defines
one configuration, description, ordered spec list, image path and physical units
for each model. Counts come from those unit records. New unavailable units use
maintenance status; the seed does not create reservations.

Run `python3 scripts/catalogue.py` after changing the data. Commit the generated
`demo_seed.sql` with it. `make catalogue-check` checks the SQL and image files.
The existing migration job runs the seed when `SEED_DEMO_DATA=true`.

Repeat runs add missing models. They do not reset existing units, counts or loans.
For the four old models, the seed updates the known old descriptions and fills
missing image/spec fields. It keeps staff-edited descriptions and reports a
warning instead of adding specs that might describe a different configuration.
An existing model never receives extra physical units from a seed rerun.

Images live in `frontend/public/catalogue/`. `sources.json` records each download
URL, source page, author, licence or rights status, and access date. The app's
`/image-credits` page shows the credits. Files use WebP, at most 960 × 640 pixels.
Some manufacturer and retailer photos have no stated open licence; the records
say so. No image-generation output is used: the built-in service returned 404.
Photos show the main product; listed kit accessories need not appear in them.
