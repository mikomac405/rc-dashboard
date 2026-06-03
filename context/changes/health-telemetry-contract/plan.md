# Health Telemetry Contract Implementation Plan

## Overview

Replace the current race/pit telemetry payload with a scouting-oriented health telemetry contract. The change aligns simulator publishing, backend ingestion/storage/API responses, and frontend rendering around the MVP's health-state decisions: `healthy`, `unhealthy`, and `dead`, with enum-like reasons, raw diagnostic signals, mission area context, manual-pickup flags, and backend-derived stale-heartbeat dead state.

## Current State Analysis

The frame brief confirmed that the current telemetry model is the wrong domain model. The simulator emits race-style samples (`lap`, `tire_wear`, temperatures, `speed_kph`, `pit_recommended`), the backend persists and returns that same shape, and the frontend renders "Race Control", lap, pit alerts, and Pit/Stay out decisions. The PRD instead requires telemetry-derived vehicle health decisions for scouting-area operations, including lost-connection dead marking and jammed manual-pickup behavior.

Auth is already present from F-01: `GET /api/telemetry` requires a normal fleet-manager bearer token, while MQTT remains the normal telemetry ingest path. HTTP ingest is reserved for explicit manual/automated contract tests through `/api/simulator/ingest`.

## Desired End State

The simulator publishes health-oriented readings that include the minimum MVP raw signals and derived state fields: vehicle identity, health state, enum-like reason, battery percent, heartbeat age, motor temperature, commanded speed, actual speed, jam detection, mission area, manual-pickup requirement, and timestamp. Race-only concepts such as `lap`, `tire_wear`, and `pit_recommended` are removed.

The backend accepts the new contract through MQTT and HTTP ingest, stores it in the inline Postgres schema and in-memory fallback, returns it from the protected telemetry API, and derives `dead` when a vehicle's heartbeat is stale or a known vehicle stops publishing beyond the freshness threshold. The contract remains compatible with a future MQTT Last Will signal without requiring full LWT behavior in this foundation if timeout-based dead marking already proves the MVP path.

The frontend becomes a contract-proof dashboard: it keeps the existing login gate and polling behavior, but shows health state, reason, raw signals, mission area, speed mismatch, jam status, and manual-pickup status instead of race and pit terminology.

### Key Discoveries:

- `context/changes/health-telemetry-contract/frame.md` confirms the initial framing: the problem is end-to-end contract mismatch, not a UI-only label issue.
- `context/foundation/prd.md` requires healthy/unhealthy/dead distinctions, stale-connection dead marking, jammed unhealthy manual pickup, and status badges derived from battery, heartbeat, motor temperature, speed mismatch, and jam detection.
- `backend/src/main.rs:60` defines the backend `TelemetryReading` with the old race/pit fields.
- `backend/src/main.rs:315` creates the inline `telemetry_readings` table with the same old fields; there are no file migrations.
- `backend/src/main.rs:259` protects `GET /api/telemetry` with fleet-manager auth and should keep that boundary.
- `simulator/src/main.rs:8` duplicates the old wire contract and `simulator/src/main.rs:71` generates lap-based samples.
- `frontend/src/App.tsx:7` duplicates the old contract in TypeScript and `frontend/src/App.tsx:275` renders race/pit dashboard metrics.

## What We're NOT Doing

- No full S-01 fleet overview, selected vehicle details, recall action, or logged-in intervention flow.
- No physical vehicle integration, real scouting map, historical telemetry analysis, or severity ranking.
- No external schema/migration framework; this plan follows the current inline startup schema pattern.
- No service-token auth for `/api/simulator/ingest` and no change to the F-01 fleet-manager login model.
- No mandatory MQTT Last Will implementation if timeout-based stale-heartbeat detection proves dead-state behavior; the contract should remain ready for LWT events.
- No multi-reason diagnostics; MVP reasons are enum-like single reason strings.

## Implementation Approach

Treat the backend as the canonical rule and storage boundary while keeping the wire shape duplicated where the repo already duplicates it. First, define the new telemetry types and persistence contract in the backend, including health reason/state enums and stale-heartbeat dead derivation. Then update the simulator to generate representative scouting-area readings. Finally, update the frontend to render a contract-proof health stream that exercises every field needed by later roadmap slices.

## Critical Implementation Details

### Startup Schema Replacement

The backend currently uses `CREATE TABLE IF NOT EXISTS`, so changing column definitions alone will not alter an existing local Postgres volume. The implementer should either add explicit lightweight `ALTER TABLE`/replacement handling in `migrate_database` or document the local dev reset step clearly in manual verification; do not silently assume existing volumes receive the new columns.

### Dead State Derivation

Backend timeout derivation is the source of truth for `dead` in this plan. Simulator readings should provide heartbeat age/raw signal inputs, and the backend must also account for known vehicles that stop publishing entirely. A stale heartbeat or missing-message freshness timeout must produce `health_state = dead`, `health_reason = lost_connection`, and `manual_pickup_required = true` before API return.

### LWT-Compatible Boundary

The contract should leave room for explicit disconnect events, such as a future MQTT Last Will message, without requiring that event path to exist before F-02 is complete. If LWT is cheap to include, it may be implemented as a narrow add-on; if not, timeout behavior remains the required MVP proof.

## Phase 1: Contract and Backend Health State

### Overview

Replace the backend telemetry shape, storage schema, load/insert paths, and health-state derivation so the API can preserve the new contract and mark stale-heartbeat vehicles as dead.

### Changes Required:

#### 1. Backend Telemetry Types

**File**: `backend/src/main.rs`

**Intent**: Replace the race/pit `TelemetryReading` model with the scouting health contract and structured state/reason values.

**Contract**: The serialized API/MQTT payload includes `vehicle_id`, `health_state`, `health_reason`, `battery_percent`, `heartbeat_age_ms`, `motor_temp_c`, `commanded_speed_kph`, `actual_speed_kph`, `jam_detected`, `mission_area`, `manual_pickup_required`, and `timestamp_ms`. State values are `healthy`, `unhealthy`, and `dead`. Reason values are fixed snake_case strings including `none`, `low_battery`, `overheated_motor`, `speed_mismatch`, `jammed`, and `lost_connection`.

#### 2. Backend Health Evaluation

**File**: `backend/src/main.rs`

**Intent**: Centralize the MVP rule so ingested readings are normalized before persistence and API return.

**Contract**: Backend ingestion derives or validates `health_state`, `health_reason`, and `manual_pickup_required` from raw signals. Stale heartbeat produces `dead` + `lost_connection` + manual pickup; known vehicles that stop publishing beyond the freshness threshold also appear as `dead` + `lost_connection` + manual pickup on API reads; jam detection produces `unhealthy` + `jammed` + manual pickup; low battery, overheated motor, or speed mismatch produce `unhealthy` with manual pickup false unless jammed/dead also applies.

#### 3. Backend Persistence Schema

**File**: `backend/src/main.rs`

**Intent**: Update the inline `telemetry_readings` table schema to store the new health contract.

**Contract**: The table can store every new payload field and no longer depends on `lap`, `tire_wear`, or `pit_recommended`. The existing `timestamp_ms DESC` index remains or is recreated for recent-reading queries. Existing local database volumes are handled explicitly via schema alteration, table replacement for local MVP data, or documented reset.

#### 4. Load and Insert Paths

**File**: `backend/src/main.rs`

**Intent**: Keep database and in-memory fallback behavior aligned with the new contract.

**Contract**: `insert_reading`, `load_recent_readings`, `persist_reading`, MQTT ingestion, `POST /api/simulator/ingest`, and protected `GET /api/telemetry` all use the same health-oriented `TelemetryReading`. `POST /api/telemetry` is removed so `/api/telemetry` is a read-only dashboard API; `/api/simulator/ingest` remains the explicit HTTP injection path for manual and automated contract tests. The backend tracks enough per-vehicle freshness state, from recent persisted or in-memory readings, for `GET /api/telemetry` to surface a dead/lost_connection/manual-pickup reading when a known vehicle stops publishing. `GET /api/telemetry` remains fleet-manager protected; health and simulator ingest boundaries remain unchanged.

#### 5. Backend Tests

**File**: `backend/src/main.rs`

**Intent**: Add focused tests for health derivation and route boundary preservation.

**Contract**: Tests cover stale heartbeat mapping to dead/lost_connection/manual pickup, missing-message freshness mapping for a known vehicle, jammed mapping to unhealthy/jammed/manual pickup, healthy readings staying healthy/none/no pickup, and the existing protected telemetry/public health behavior still passing.

### Success Criteria:

#### Automated Verification:

- Backend compiles: `cargo build --manifest-path backend/Cargo.toml`
- Backend tests pass: `cargo test --manifest-path backend/Cargo.toml`
- Backend tests prove stale heartbeat produces dead/lost_connection/manual pickup.
- Backend tests prove a known vehicle with no fresh messages is surfaced as dead/lost_connection/manual pickup.
- Backend tests prove jam detection produces unhealthy/jammed/manual pickup.
- Existing route boundary tests still prove `GET /api/telemetry` rejects missing bearer tokens, `POST /api/telemetry` is not an ingest route, and `/health` remains public.

#### Manual Verification:

- A sample POST to `/api/simulator/ingest` with new contract fields is accepted by the backend.
- Protected `GET /api/telemetry` returns new contract field names and no race-only fields.
- Protected `GET /api/telemetry` can surface a dead/lost_connection/manual-pickup state for a known vehicle after message freshness expires.
- Local database schema behavior is understood: either existing volumes migrate cleanly or the reset step is documented before implementation proceeds.

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the backend contract and stale-heartbeat behavior are correct before proceeding to the next phase.

---

## Phase 2: Simulator Health Scenarios

### Overview

Replace lap-based simulator generation with scouting-area telemetry scenarios that exercise healthy, unhealthy, jammed, and stale-heartbeat/dead cases through MQTT.

### Changes Required:

#### 1. Simulator Telemetry Type

**File**: `simulator/src/main.rs`

**Intent**: Keep the simulator wire payload aligned with the backend contract.

**Contract**: The simulator `TelemetryReading` serializes the same field names and enum-like string values expected by the backend. Remove `lap`, `tire_wear`, and `pit_recommended`; add mission/scouting and health signal fields.

#### 2. Scenario Generation

**File**: `simulator/src/main.rs`

**Intent**: Generate repeatable, easy-to-recognize health scenarios for manual and API verification.

**Contract**: Sample generation cycles through representative mission areas and health states: healthy scouting, low battery or overheated motor unhealthy, speed mismatch unhealthy, jammed unhealthy/manual pickup, and stale-heartbeat input that the backend turns into dead/lost_connection/manual pickup. The old lap counter is replaced with non-racing progression such as a sample counter or mission-area cycle.

#### 3. Publish Logging

**File**: `simulator/src/main.rs`

**Intent**: Make simulator logs useful for verifying health scenarios without dumping entire JSON payloads every loop.

**Contract**: Published log lines include vehicle ID, mission area, health state/reason, battery, motor temperature, jam status, and manual pickup or stale-heartbeat signal. Logs should not mention laps, tire wear, pit, or race calls.

#### 4. Simulator Verification

**File**: `simulator/src/main.rs`

**Intent**: Add lightweight tests or deterministic helper coverage where practical.

**Contract**: If helper functions are introduced for sample generation or health scenario cycling, cover them with unit tests. If the crate remains too small for meaningful tests beyond compilation, document simulator verification through `cargo build --manifest-path simulator/Cargo.toml` and local MQTT smoke testing.

### Success Criteria:

#### Automated Verification:

- Simulator compiles: `cargo build --manifest-path simulator/Cargo.toml`
- Simulator tests pass if testable helpers are added: `cargo test --manifest-path simulator/Cargo.toml`
- Backend still compiles after simulator/backend contract alignment: `cargo build --manifest-path backend/Cargo.toml`

#### Manual Verification:

- Simulator logs show scouting/health terminology and no lap/pit terminology.
- Running simulator plus backend produces multiple health states through the MQTT path.
- Backend API returns simulator-produced healthy, unhealthy, jammed, and dead/lost_connection samples.

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that simulator scenarios represent the scouting domain correctly before proceeding to the next phase.

---

## Phase 3: Frontend Contract Proof Dashboard

### Overview

Replace the frontend's race/pit dashboard fields with a health-focused telemetry stream that proves the new contract survives to the browser without building the full fleet overview slice.

### Changes Required:

#### 1. Frontend Telemetry Type

**File**: `frontend/src/App.tsx`

**Intent**: Align the TypeScript telemetry type with the backend API response.

**Contract**: `TelemetryReading` uses the new health contract fields and no longer references `lap`, `tire_wear`, or `pit_recommended`.

#### 2. Summary Metrics

**File**: `frontend/src/App.tsx`

**Intent**: Show health-focused proof metrics in the existing dashboard structure.

**Contract**: Replace Current car/Lap/Speed/Pit alerts with metrics such as current vehicle, health state, manual pickup count, and active mission area or stale/dead count. The dashboard title should no longer be "Race Control".

#### 3. Telemetry Table

**File**: `frontend/src/App.tsx`

**Intent**: Render every contract field needed to manually verify health decisions.

**Contract**: The table shows vehicle, mission area, health state badge, reason, battery, motor temperature, commanded speed, actual speed or mismatch, jam status, manual-pickup flag, and timestamp or sample age. It stays a contract-proof stream, not a selected-vehicle details view.

#### 4. Styling for Health States

**File**: `frontend/src/App.css`

**Intent**: Style health state and manual-pickup indicators using the existing restrained dashboard style.

**Contract**: Add state-specific classes for healthy, unhealthy, dead, jammed/manual-pickup indicators as needed. Preserve responsive table behavior and login/password-change styles from F-01.

### Success Criteria:

#### Automated Verification:

- Frontend build passes: `npm run build` in `frontend/`.
- Frontend lint passes: `npm run lint` in `frontend/`.
- Backend build still passes: `cargo build --manifest-path backend/Cargo.toml`

#### Manual Verification:

- Logged-in dashboard shows health/scouting terminology and no lap, race, pit, tire-wear, or stay-out terminology.
- Healthy, unhealthy, dead, and jammed/manual-pickup rows are visually distinguishable in the stream.
- Expired or missing auth still returns the user to login instead of showing telemetry.
- The UI remains usable at existing desktop and narrow responsive widths.

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the contract-proof dashboard is clear and does not overreach into S-01/S-02 UX before proceeding to the next phase.

---

## Phase 4: End-to-End Verification and Docs Alignment

### Overview

Verify the full simulator-to-dashboard path and update local project guidance so future agents keep telemetry fields aligned.

### Changes Required:

#### 1. Environment and Compose Check

**File**: `.env.example`

**Intent**: Confirm local telemetry configuration remains accurate for the new contract.

**Contract**: Keep MQTT and auth env names aligned with the current services. Add no new env variables unless implementation actually introduces one for heartbeat/dead thresholds or simulator scenario tuning.

#### 2. Compose Flow

**File**: `docker-compose.yml`

**Intent**: Preserve the existing local stack wiring while the telemetry payload changes.

**Contract**: Backend, simulator, MQTT topic, Postgres, and frontend service boundaries stay intact. Only update compose if implementation adds explicit threshold/scenario env variables.

#### 3. Repository Guidance

**File**: `AGENTS.md`

**Intent**: Keep the hard rule about duplicated telemetry fields accurate after the contract changes.

**Contract**: If field names are listed or implied by old race/pit terminology, update the guidance to reference the new health-state contract. Do not write under `context/archive/`.

#### 4. Final Verification Pass

**File**: `context/changes/health-telemetry-contract/plan.md`

**Intent**: Keep implementation verification tied to the canonical progress section.

**Contract**: `/10x-implement` marks the `## Progress` checklist as phases land. No sidecar state files are added.

### Success Criteria:

#### Automated Verification:

- Backend build passes: `cargo build --manifest-path backend/Cargo.toml`
- Backend tests pass: `cargo test --manifest-path backend/Cargo.toml`
- Simulator build passes: `cargo build --manifest-path simulator/Cargo.toml`
- Simulator tests pass if present: `cargo test --manifest-path simulator/Cargo.toml`
- Frontend build passes: `npm run build` in `frontend/`.
- Frontend lint passes: `npm run lint` in `frontend/`.

#### Manual Verification:

- `docker compose up --build` starts backend, frontend, simulator, Postgres, and MQTT with the new contract.
- After login, the dashboard receives simulator readings through MQTT/backend/API and shows health states, reasons, mission areas, and manual-pickup flags.
- A stale-heartbeat simulator case appears as dead/lost_connection/manual pickup in the dashboard.
- `AGENTS.md` and environment docs no longer imply the old race/pit contract.

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the end-to-end flow proves F-02 before moving to later roadmap slices.

---

## Testing Strategy

### Unit Tests:

- Backend health evaluation for healthy, low battery, overheated motor, speed mismatch, jammed, stale-heartbeat dead, and missing-message dead cases.
- Backend route-boundary tests to preserve protected `GET /api/telemetry` and public `/health`.
- Simulator sample generation tests if helper functions are extracted.

### Integration Tests:

- Backend ingest then protected telemetry read returns the new contract and derived health fields.
- Protected telemetry read marks a known vehicle dead when no fresh message has arrived within the MVP freshness threshold.
- MQTT simulator publishing flows through backend persistence and appears in recent readings.
- Existing auth tests continue to pass after telemetry contract changes.

### Manual Testing Steps:

1. Build and test backend, simulator, and frontend with the commands listed in phase success criteria.
2. Run the local stack with `docker compose up --build`.
3. Log in through the frontend and confirm telemetry polling succeeds.
4. Watch simulator logs and dashboard rows for healthy, unhealthy, jammed, and stale-heartbeat/dead scenarios.
5. Confirm no user-facing race/lap/pit terminology remains in the dashboard.

## Performance Considerations

The MVP scale is small and `MAX_READINGS` remains 120, so the wider payload should not require pagination, caching, or indexing beyond the existing timestamp ordering. Stale-heartbeat and missing-message derivation should be simple and bounded to known recent vehicles; avoid adding a broad background scanner unless needed to meet the backend-timeout behavior.

## Migration Notes

This repo currently uses inline startup schema creation, not SQL migration files. Existing local Postgres volumes may retain the old `telemetry_readings` columns if only `CREATE TABLE IF NOT EXISTS` is edited. The implementation must either include explicit schema transition logic suitable for MVP local data or document that developers should reset the local Postgres volume when applying F-02.

## References

- Frame brief: `context/changes/health-telemetry-contract/frame.md`
- PRD: `context/foundation/prd.md`
- Roadmap: `context/foundation/roadmap.md`
- Backend telemetry contract: `backend/src/main.rs:60`
- Backend telemetry schema: `backend/src/main.rs:315`
- Backend protected telemetry route: `backend/src/main.rs:259`
- Simulator telemetry contract: `simulator/src/main.rs:8`
- Frontend telemetry contract: `frontend/src/App.tsx:7`
- Prior plan style: `context/changes/minimal-login-gate/plan.md`

## Progress

> Convention: `- [ ]` pending, `- [x]` done. Append ` — <commit sha>` when a step lands. Do not rename step titles. See `references/progress-format.md`.

### Phase 1: Contract and Backend Health State

#### Automated

- [x] 1.1 Backend compiles: `cargo build --manifest-path backend/Cargo.toml` — 69109e8
- [x] 1.2 Backend tests pass: `cargo test --manifest-path backend/Cargo.toml` — 69109e8
- [x] 1.3 Backend tests prove stale heartbeat produces dead/lost_connection/manual pickup. — 69109e8
- [x] 1.4 Backend tests prove a known vehicle with no fresh messages is surfaced as dead/lost_connection/manual pickup. — 69109e8
- [x] 1.5 Backend tests prove jam detection produces unhealthy/jammed/manual pickup. — 69109e8
- [x] 1.6 Existing route boundary tests still prove `GET /api/telemetry` rejects missing bearer tokens, `POST /api/telemetry` is not an ingest route, and `/health` remains public. — 69109e8

#### Manual

- [x] 1.7 A sample POST to `/api/simulator/ingest` with new contract fields is accepted by the backend. — 69109e8
- [x] 1.8 Protected `GET /api/telemetry` returns new contract field names and no race-only fields. — 69109e8
- [x] 1.9 Protected `GET /api/telemetry` can surface a dead/lost_connection/manual-pickup state for a known vehicle after message freshness expires. — 69109e8
- [x] 1.10 Local database schema behavior is understood: either existing volumes migrate cleanly or the reset step is documented before implementation proceeds. — 69109e8

### Phase 2: Simulator Health Scenarios

#### Automated

- [x] 2.1 Simulator compiles: `cargo build --manifest-path simulator/Cargo.toml` — 69109e8
- [x] 2.2 Simulator tests pass if testable helpers are added: `cargo test --manifest-path simulator/Cargo.toml` — 69109e8
- [x] 2.3 Backend still compiles after simulator/backend contract alignment: `cargo build --manifest-path backend/Cargo.toml` — 69109e8

#### Manual

- [x] 2.4 Simulator logs show scouting/health terminology and no lap/pit terminology. — 2ce3ff4
- [x] 2.5 Running simulator plus backend produces multiple health states through the MQTT path. — 2ce3ff4
- [x] 2.6 Backend API returns simulator-produced healthy, unhealthy, jammed, and dead/lost_connection samples. — 2ce3ff4

### Phase 3: Frontend Contract Proof Dashboard

#### Automated

- [x] 3.1 Frontend build passes: `npm run build` in `frontend/`.
- [x] 3.2 Frontend lint passes: `npm run lint` in `frontend/`.
- [x] 3.3 Backend build still passes: `cargo build --manifest-path backend/Cargo.toml`

#### Manual

- [x] 3.4 Logged-in dashboard shows health/scouting terminology and no lap, race, pit, tire-wear, or stay-out terminology.
- [x] 3.5 Healthy, unhealthy, dead, and jammed/manual-pickup rows are visually distinguishable in the stream.
- [x] 3.6 Expired or missing auth still returns the user to login instead of showing telemetry.
- [x] 3.7 The UI remains usable at existing desktop and narrow responsive widths.

### Phase 4: End-to-End Verification and Docs Alignment

#### Automated

- [ ] 4.1 Backend build passes: `cargo build --manifest-path backend/Cargo.toml`
- [ ] 4.2 Backend tests pass: `cargo test --manifest-path backend/Cargo.toml`
- [ ] 4.3 Simulator build passes: `cargo build --manifest-path simulator/Cargo.toml`
- [ ] 4.4 Simulator tests pass if present: `cargo test --manifest-path simulator/Cargo.toml`
- [ ] 4.5 Frontend build passes: `npm run build` in `frontend/`.
- [ ] 4.6 Frontend lint passes: `npm run lint` in `frontend/`.

#### Manual

- [ ] 4.7 `docker compose up --build` starts backend, frontend, simulator, Postgres, and MQTT with the new contract.
- [ ] 4.8 After login, the dashboard receives simulator readings through MQTT/backend/API and shows health states, reasons, mission areas, and manual-pickup flags.
- [ ] 4.9 A stale-heartbeat simulator case appears as dead/lost_connection/manual pickup in the dashboard.
- [ ] 4.10 `AGENTS.md` and environment docs no longer imply the old race/pit contract.
