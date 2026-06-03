# Health Telemetry Contract — Plan Brief

> Full plan: `context/changes/health-telemetry-contract/plan.md`
> Frame brief: `context/changes/health-telemetry-contract/frame.md`

## What & Why

The simulator-to-dashboard telemetry contract still models race pit decisions instead of MVP vehicle health decisions, so health state, reasons, and action eligibility cannot survive the end-to-end data path.

This plan replaces the old race/pit contract with a scouting-oriented health telemetry contract that can distinguish healthy, unhealthy, dead, and jammed vehicles before later overview/details/action slices build on it.

## Starting Point

Today, simulator, backend, persistence, and frontend all carry `lap`, `tire_wear`, `speed_kph`, and `pit_recommended`. The frame brief confirmed this is an end-to-end contract problem, not just a frontend label mismatch.

## Desired End State

Simulator readings, backend storage/API responses, and the frontend all share the same health contract: vehicle ID, health state, enum reason, battery percent, heartbeat age, motor temperature, commanded and actual speed, jam detection, mission area, manual-pickup flag, and timestamp. Backend logic derives dead/lost_connection/manual-pickup from stale heartbeat input or missing messages from a known vehicle. The frontend shows a contract-proof health stream without building the full S-01 fleet overview.

## Key Decisions Made

| Decision | Choice | Why | Source |
| --- | --- | --- | --- |
| Problem framing | Cross-system contract mismatch | Every telemetry surface carries the old race/pit payload. | Frame |
| Domain correction | Remove `lap` and race concepts | Vehicles scout/analyze areas rather than race laps. | Frame |
| Contract breadth | Health output + raw signals + mission area | Proves both rule inputs and outputs survive end to end. | Plan |
| Health reasons | Fixed snake_case reason values | Keeps storage, tests, and UI rendering predictable. | Plan |
| Dead behavior | Backend derives dead from stale heartbeat or missing messages | Matches lost-connection semantics better than simulator-only dead payloads. | Plan |
| MQTT Last Will | LWT-compatible, timeout first | Leaves room for explicit disconnect events without blocking MVP proof. | Plan |
| Frontend scope | Contract-proof dashboard | Verifies the contract without stealing S-01/S-02 UX scope. | Plan |

## Scope

**In scope:**

- Backend health telemetry type, schema, persistence, API response, and health derivation.
- Simulator health/scouting scenarios for healthy, unhealthy, jammed, and stale-heartbeat cases.
- Frontend health stream rendering and status styling.
- Verification across Rust builds/tests, frontend build/lint, and local Docker Compose smoke path.
- Guidance/doc updates when old race/pit terminology would mislead future agents.

**Out of scope:**

- Full fleet overview, selected vehicle details, recall actions, intervention flow.
- Physical vehicle integration, map UI, historical analysis, severity ranking.
- Mandatory MQTT Last Will implementation if timeout behavior proves dead state.
- New migration framework or auth model changes.

## Architecture / Approach

Data flow remains `simulator -> MQTT -> backend ingest -> Postgres/in-memory recent readings -> protected GET /api/telemetry -> frontend`. The backend becomes the canonical health-rule boundary, normalizing raw signals into state/reason/manual-pickup fields before storage and API return. `POST /api/simulator/ingest` is the explicit HTTP injection path for manual and automated contract tests; `/api/telemetry` is read-only dashboard API.

## Phases at a Glance

| Phase | What it delivers | Key risk |
| --- | --- | --- |
| 1. Contract and Backend Health State | New backend contract, schema, persistence, health derivation, tests | Existing local Postgres volumes may keep old columns |
| 2. Simulator Health Scenarios | Scouting-oriented MQTT samples covering health states | Scenario data may feel artificial if too terse |
| 3. Frontend Contract Proof Dashboard | Browser proof that all contract fields survive | Accidentally expanding into S-01/S-02 UX |
| 4. End-to-End Verification and Docs Alignment | Full-stack smoke and guidance cleanup | Compose/manual verification may reveal schema reset needs |

**Prerequisites:** F-01 login gate is present enough that protected telemetry reads already use bearer auth.
**Estimated effort:** ~3-4 focused implementation sessions across 4 phases.

## Open Risks & Assumptions

- Inline schema changes need explicit local-volume handling because `CREATE TABLE IF NOT EXISTS` will not reshape existing tables.
- Stale heartbeat and missing-message thresholds can be chosen conservatively during implementation; they are MVP simulation values, not physical-vehicle calibration.
- LWT remains a compatibility boundary unless implementation proves it is cheap and reliable to add now.

## Success Criteria (Summary)

- Protected telemetry returns health/scouting fields and no race-only fields.
- Simulator-to-dashboard flow visibly shows healthy, unhealthy, jammed/manual-pickup, and dead/lost_connection cases.
- Backend, simulator, and frontend verification commands pass, and docs no longer imply the old race/pit contract.
