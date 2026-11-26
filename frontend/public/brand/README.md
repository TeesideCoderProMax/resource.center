# Campus Device Hub logo

The mark combines a device screen with a handoff arrow. It uses Oxford Navy
(#172554), white, and Vivid Tangerine (#F97316). Keep clear space around the mark. Do not
stretch it or change its proportions.

- `logo.svg`: full logo with the Resource Center label; use on light backgrounds.
- `logo.png`: transparent 1296 × 240 export of the full logo.
- `logo-mark.svg`: square mark used in the app header.
- `logo-mark.png`: transparent 512 × 512 mark export.

Browser icons use the same mark: `src/app/icon.svg`, a 16/32/48 px
`src/app/favicon.ico`, and a 180 px `src/app/apple-icon.png`. Next.js adds their
metadata links. When changing the mark, update those files from logo-mark.svg.

These are original project SVG assets. They need no outside image attribution.

## App palette

The UI theme lives in `src/app/globals.css` as CSS tokens (light and dark):
Oxford Navy (`#0f172a`, `#172554`) for text, headers and accents; Vivid Tangerine
(`#ea580c`, hover `#f97316`) for primary actions, checkboxes and count badges;
slate/porcelain surfaces. Status badges: emerald for available, tangerine for
low stock, slate navy for waitlist.
