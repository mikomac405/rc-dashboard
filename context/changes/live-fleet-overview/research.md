---
date: 2026-06-03T12:05:22+02:00
researcher: Codex
git_commit: 5f48a0e8e36bf2e6abe555a6e4435062b910c931
branch: feature/live-fleet-overview
repository: rc-dashboard
topic: "Visual library choice for live fleet overview"
tags: [research, codebase, frontend, telemetry, visualization]
status: complete
last_updated: 2026-06-03
last_updated_by: Codex
---

# Research: Visual library choice for live fleet overview

**Date**: 2026-06-03T12:05:22+02:00
**Researcher**: Codex
**Git Commit**: 5f48a0e8e36bf2e6abe555a6e4435062b910c931
**Branch**: feature/live-fleet-overview
**Repository**: rc-dashboard

## Research Question

`/10x-research live-fleet-overview I want add some visuals here but I'm not sure which library should I use to present it`

## Summary

For S-01, do not start with a heavy visualization library. The current product need is a protected, live, fleet-level scanning view, not analytics. Build the core visuals with React, semantic HTML, and CSS: fleet summary counts, one latest tile per vehicle, health-state badges, manual-pickup markers, mission-area grouping, battery bars, heartbeat age, and speed mismatch indicators.

If iconography is useful, add `lucide-react` as a small visual helper. Lucide is an icon package, not a charting system; it fits state glyphs such as healthy, warning, dead/lost connection, manual pickup, and logout/action icons.

If the plan decides S-01 needs real charts, choose `recharts`. It is React-native in shape, current official docs list v3.8.1, and it supports composable SVG charts with responsive containers. Use it for small fleet charts such as health distribution, mission-area counts, or battery spread. Avoid ECharts, Nivo, Visx, Chart.js, map libraries, or a component library for this slice unless the scope changes.

The main implementation risk is not library choice. It is ensuring the overview derives one canonical latest state per vehicle from a recent-samples API that can include duplicate vehicle rows.

## Detailed Findings

### Product Scope

- S-01 is `live-fleet-overview`: a protected fleet overview fed by simulator telemetry ([context/foundation/roadmap.md:89](/home/mmacura/Documents/Git/rc-dashboard/context/foundation/roadmap.md:89)).
- The overview must let the fleet manager identify unhealthy and dead vehicles ([context/foundation/prd.md:57](/home/mmacura/Documents/Git/rc-dashboard/context/foundation/prd.md:57)).
- Dead, movable unhealthy, and jammed unhealthy vehicles must be visibly distinct before action ([context/foundation/prd.md:83](/home/mmacura/Documents/Git/rc-dashboard/context/foundation/prd.md:83)).
- The business rule outputs `healthy`, `unhealthy`, or `dead`; jammed vehicles stay unhealthy with a jammed reason and manual pickup ([context/foundation/prd.md:92](/home/mmacura/Documents/Git/rc-dashboard/context/foundation/prd.md:92)).
- Non-goals rule out historical vehicle status overview, severity ranking, physical vehicle integration, and mobile dashboard support for the MVP ([context/foundation/prd.md:108](/home/mmacura/Documents/Git/rc-dashboard/context/foundation/prd.md:108)).

Implication: visuals should optimize current-state scanning. Do not build historical charts, severity dashboards, maps, or selected-vehicle details inside S-01.

### Frontend Baseline

- The frontend has only React and React DOM as runtime dependencies; no chart, map, table, icon, router, or component library is installed ([frontend/package.json:12](/home/mmacura/Documents/Git/rc-dashboard/frontend/package.json:12)).
- The dashboard lives in one React component file and uses a local `TelemetryReading` type with the current health contract ([frontend/src/App.tsx:7](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.tsx:7), [frontend/src/App.tsx:44](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.tsx:44)).
- Authenticated telemetry polling calls `/api/telemetry` every 2500 ms with `Authorization: Bearer ...` ([frontend/src/App.tsx:147](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.tsx:147), [frontend/src/App.tsx:187](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.tsx:187)).
- The current dashboard already has summary cards and a telemetry table ([frontend/src/App.tsx:289](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.tsx:289), [frontend/src/App.tsx:308](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.tsx:308)).
- Visual primitives are CSS cards, status pills, health badges, and flags, not reusable component modules yet ([frontend/src/App.css:114](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.css:114), [frontend/src/App.css:136](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.css:136), [frontend/src/App.css:143](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.css:143)).
- Global CSS already provides restrained light/dark tokens for status colors ([frontend/src/index.css:1](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/index.css:1)).
- TypeScript rejects unused locals and parameters, so added derived-data helpers must be used cleanly ([frontend/tsconfig.app.json:18](/home/mmacura/Documents/Git/rc-dashboard/frontend/tsconfig.app.json:18)).
- Vite proxies `/api` to the backend locally, so visualization code can keep using relative API URLs ([frontend/vite.config.ts:7](/home/mmacura/Documents/Git/rc-dashboard/frontend/vite.config.ts:7)).

Implication: S-01 can get a much clearer UI by refactoring data derivation and CSS layout before adding a data-viz dependency.

### Backend and Telemetry Constraints

- `GET /api/telemetry` is protected by a normal fleet-manager bearer session ([backend/src/main.rs:288](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:288)).
- `/api/simulator/ingest` is public and normalizes submitted telemetry before persistence ([backend/src/main.rs:206](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:206), [backend/src/main.rs:310](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:310)).
- The backend health contract includes `vehicle_id`, health state/reason, battery, heartbeat age, motor temperature, commanded/actual speed, jam flag, mission area, manual pickup, and timestamp ([backend/src/main.rs:64](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:64)).
- DB storage mirrors the current health contract fields ([backend/src/main.rs:348](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:348)).
- The simulator publishes the same core health fields and cycles representative health scenarios ([simulator/src/main.rs:27](/home/mmacura/Documents/Git/rc-dashboard/simulator/src/main.rs:27), [simulator/src/main.rs:98](/home/mmacura/Documents/Git/rc-dashboard/simulator/src/main.rs:98)).
- Freshness handling synthesizes `dead/lost_connection/manual_pickup_required` when a known vehicle's latest sample is stale ([backend/src/main.rs:613](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:613)).
- MQTT messages are persisted without calling `normalize_reading`; HTTP ingest normalizes first. The normal simulator path therefore trusts simulator-provided state/reason/manual-pickup values ([backend/src/main.rs:821](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:821), [backend/src/main.rs:841](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:841)).

Implication: the overview should treat the API response as already classified, but should compute display grouping carefully. It should derive latest-per-vehicle before counts, tiles, and visual summaries.

### Library Recommendation

Recommended default: no charting library for the core S-01 overview.

Use custom React/CSS for:

- one latest vehicle card per `vehicle_id`
- state count strip: healthy, unhealthy, dead, manual pickup
- mission-area grouping
- battery meter bars
- heartbeat age and stale/dead emphasis
- speed mismatch indicators
- jam/manual-pickup flags

Optional small visual helper: `lucide-react`.

- Lucide's official site describes lightweight scalable SVG icons, package support, tree shaking, and customization options (https://lucide.dev/).
- This is appropriate for status glyphs and controls, not telemetry charting.

Recommended chart library only if S-01 needs charts: `recharts`.

- Recharts official docs describe it as a composable charting library built on React components, with v3.8.1 current on the docs page and MIT licensing (https://recharts.github.io/).
- Recharts uses SVG and D3 submodules and supports customization through component props and custom components (https://recharts.github.io/).
- `ResponsiveContainer` handles container-based sizing with `ResizeObserver`, but chart parents need stable dimensions (https://recharts-recharts.mintlify.app/api/components/responsive-container).

When to add `recharts`:

- You need a compact health distribution chart, battery spread chart, or mission-area count chart.
- You can define stable chart container dimensions in CSS.
- You are willing to update `frontend/package.json` and the lockfile, then run `npm run build` and `npm run lint`.

When not to add `recharts`:

- The visual can be a card, badge, meter, progress bar, or grouped list.
- The chart would duplicate the same information as the vehicle tiles.
- The chart makes the overview feel like historical analytics, which is a parked MVP concern.

Rejected for S-01 unless scope changes:

- Apache ECharts: very capable and supports many chart types, Canvas/SVG, progressive rendering, and realtime-scale data (https://echarts.apache.org/en/index.html), but that is more power and API surface than this small current-state dashboard needs.
- Nivo: strong dataviz library, but heavier than the current need and more useful when many chart families are required.
- Visx: low-level primitives are useful when visualization itself is a product differentiator; S-01 needs fast, clear operational scanning.
- Chart.js/react-chartjs-2: good generic charts, but less aligned with React component composition than Recharts for this repo's current style.
- Map/diagram libraries: physical location is not in the MVP contract, and the roadmap parks physical integration.
- Full component libraries: the app has a small bespoke dashboard surface and no established design system dependency.

## Code References

- `frontend/package.json:12` - Runtime dependencies are only `react` and `react-dom`.
- `frontend/src/App.tsx:7` - Frontend duplicates the health telemetry wire contract.
- `frontend/src/App.tsx:147` - Authenticated telemetry polling flow.
- `frontend/src/App.tsx:187` - Current polling interval is 2500 ms, while the PRD says 30-second refresh cadence.
- `frontend/src/App.tsx:195` - UI currently treats `readings[0]` as latest.
- `frontend/src/App.tsx:336` - Table renders recent samples, not canonical latest-per-vehicle rows.
- `frontend/src/App.css:136` - Existing four-column summary grid.
- `frontend/src/App.css:185` - Existing table panel framing.
- `backend/src/main.rs:288` - Protected telemetry handler.
- `backend/src/main.rs:613` - Dead-state freshness synthesis.
- `simulator/src/main.rs:98` - Simulator health scenario generation.
- `context/foundation/prd.md:57` - Overview must identify unhealthy/dead vehicles.
- `context/foundation/prd.md:83` - Dead, movable unhealthy, and jammed unhealthy states must be visibly distinct.

## Architecture Insights

- The live overview should introduce a `latestByVehicle` derived collection before rendering. Counts, cards, and charts should use this collection, not raw recent samples, or one fast-publishing vehicle can distort fleet health.
- The existing table can remain as a secondary "recent stream" if useful, but the primary S-01 surface should be fleet overview.
- Keep the current auth boundary: no telemetry is rendered before authenticated state, and `401` still resets the session.
- Keep state/reason strings aligned across `backend/src/main.rs`, `simulator/src/main.rs`, and `frontend/src/App.tsx`; this is a repository hard rule.
- Build stable fixed-format visual containers. If Recharts is added, wrap charts in CSS containers with fixed `min-height`/`aspect-ratio` so responsive chart measurement does not collapse.
- The PRD says 30-second refresh cadence, but current frontend polling is 2.5 seconds. Planning should decide whether S-01 keeps the current live cadence for simulator feedback or aligns to the PRD.

## Historical Context

- `context/changes/minimal-login-gate/plan.md` established the current protected dashboard auth flow and deliberately avoided role complexity.
- `context/changes/health-telemetry-contract/plan.md` replaced race/pit telemetry with the current health-state contract and explicitly stopped before building a full fleet overview.
- `context/changes/health-telemetry-contract/frame.md` clarified that the telemetry issue was an end-to-end contract mismatch, not a UI-only rename.
- `context/foundation/roadmap.md` is partially stale in its baseline section: current code already has auth and health-state telemetry even though the baseline text says those were absent or race/pit oriented.
- No `context/foundation/lessons.md` exists.
- No prior `research.md` files were found under `context/changes/**`; no archived change history exists beyond the archive README.

## Related Research

No prior research artifacts were found for this change.

## Open Questions

- Should S-01 preserve the current 2.5-second polling interval, or change toward the PRD's 30-second refresh cadence?
- Should the API continue returning recent samples only, or should S-01 add a backend endpoint/shape for latest-per-vehicle? A frontend-only derivation is fastest, but a backend latest endpoint would make the overview contract clearer later.
- Is `lucide-react` worth adding now, or should the first S-01 plan stay dependency-free unless Recharts is explicitly needed?
- Should public `/api/simulator/ingest` stay public for MVP local testing, or should it be scoped in a later protection change before deployment?
