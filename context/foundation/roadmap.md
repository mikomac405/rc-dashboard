---
project: RC Smart Pit-Stop
version: 1
status: draft
created: 2026-05-28
updated: 2026-06-03
prd_version: 3
main_goal: speed
top_blocker: time
---

# Roadmap: RC Smart Pit-Stop

> Derived from `context/foundation/prd.md` (v3) + auto-researched codebase baseline.
> Edit-in-place; archive when superseded.
> Slices below are listed in dependency order. The "At a glance" table is the index.

## Vision recap

Fleet managers lose time and fleet reliability when they must manually inspect autonomous RC vehicle telemetry and mechanical condition to detect broken or dead vehicles. RC Smart Pit-Stop starts with a simulated fleet feeding a dashboard, so the manager can learn which telemetry and control capabilities physical vehicles need later. For this speed-biased MVP, the shortest useful path is a protected dashboard that turns simulated telemetry into clear health decisions and safe operator actions.

## North star

**S-04: Logged-in manager handles an unhealthy or dead vehicle** — This is the north star, meaning the smallest end-to-end slice whose delivery proves the product works: a logged-in manager sees a fleet issue, opens the right vehicle, applies the safe recall or pickup path, and sees status update.

## At a glance

| ID | Change ID | Outcome (user can ...) | Prerequisites | PRD refs | Status |
|---|---|---|---|---|---|
| F-01 | minimal-login-gate | (foundation) logged-in fleet manager access is enforced for dashboard data and recall paths | — | FR-001, Access Control | ready |
| F-02 | health-telemetry-contract | (foundation) simulator, backend, persistence, and frontend share the health-state telemetry contract | — | FR-003, FR-006, FR-007 | ready |
| F-03 | minimal-ci-verification | (foundation) backend, simulator, and frontend verification commands run in CI | — | NFR: automated verification | ready |
| F-04 | basic-operational-visibility | (foundation) MVP runtime has clear logs, API errors, and dashboard runtime states for troubleshooting | — | NFR: basic operational visibility | ready |
| S-01 | live-fleet-overview | user can view a protected fleet overview fed by simulator telemetry | F-01, F-02 | US-01, FR-002, FR-003, FR-007 | proposed |
| S-02 | vehicle-health-details | user can open selected vehicle details that preserve healthy, unhealthy, dead, and jammed distinctions | S-01 | US-01, FR-003, FR-004, FR-006 | proposed |
| S-03 | safe-vehicle-action | user can recall a movable unhealthy vehicle or see dead and jammed vehicles marked for manual pickup | S-02 | US-01, FR-005, FR-006 | proposed |
| S-04 | logged-in-intervention-flow | user can complete the logged-in unhealthy/dead vehicle intervention flow and see status update | S-03 | US-01, FR-001, FR-002, FR-003, FR-004, FR-005, FR-006, FR-007 | proposed |
| S-05 | manager-account-creation | user can create another fleet manager from the protected dashboard | S-04 | US-02, FR-001, FR-008, Access Control | proposed |
| S-06 | simulated-vehicle-creation | user can create a simulated vehicle from the protected dashboard and see it enter fleet monitoring | S-04 | US-03, FR-002, FR-007, FR-009 | proposed |

## Streams

Navigation aid - groups items that share a Prerequisites chain. Canonical ordering still lives in the dependency graph below; this table is the proposed reading order across parallel tracks.

| Stream | Theme | Chain | Note |
|---|---|---|---|
| A | Protected access | `F-01` | Keeps the must-have login path on the critical path, then joins Stream B at `S-01`. |
| B | Health telemetry | `F-02` -> `S-01` -> `S-02` -> `S-03` -> `S-04` | Turns the crucial simulator into the source of dashboard health decisions, then joins protected access at `S-01`. |
| C | MVP reliability | `F-03` / `F-04` | Adds verification and troubleshooting support without changing the north-star product path. |
| D | Dashboard administration | `S-05` / `S-06` | Extends manager and simulated-vehicle setup after the north-star intervention flow is proven. |

## Baseline

What's already in place in the codebase as of `2026-05-28` (auto-researched + user-confirmed).
Foundations below assume these are present and do NOT re-scaffold them.

- **Frontend:** present — React + Vite + TypeScript dashboard exists; it polls `/api/telemetry`, but has no router or component library.
- **Backend / API:** present — Axum exposes `/health`, `/api/telemetry`, and `/api/simulator/ingest`, and also consumes MQTT.
- **Simulator:** present — Rust MQTT simulator publishes `rc/telemetry`; this is crucial for the MVP, but its current payload is race/pit telemetry rather than the PRD health-state contract.
- **Data:** partial — PostgreSQL via SQLx exists with inline startup schema and in-memory fallback; no file migrations or seeded data.
- **Auth:** absent — no auth provider, session/token paths, route-level auth middleware, or frontend auth headers are implemented.
- **Deploy / infra:** partial — Dockerfiles, Docker Compose, and Railway deployment docs exist; CI/CD and infrastructure-as-code are absent.
- **Observability:** partial — ad hoc stdout/stderr logs exist; no tracing middleware, metrics, error tracking, or operational dashboard exists.

## Foundations

### F-01: Minimal login gate

- **Outcome:** (foundation) logged-in fleet manager access is enforced for dashboard data and recall paths without adding non-MVP role complexity.
- **Change ID:** minimal-login-gate
- **PRD refs:** FR-001, Access Control
- **Unlocks:** S-01, S-04, protected verification path for telemetry and recall actions
- **Prerequisites:** —
- **Parallel with:** F-02
- **Blockers:** —
- **Unknowns:** —
- **Risk:** Sequenced first because every PRD flow starts with a logged-in manager; the risk is spending time on roles instead of the flat MVP model.
- **Status:** ready

### F-02: Health telemetry contract

- **Outcome:** (foundation) simulator, backend, persistence, and frontend share the health-state telemetry contract needed to distinguish healthy, unhealthy, dead, and jammed vehicles.
- **Change ID:** health-telemetry-contract
- **PRD refs:** FR-003, FR-006, FR-007, Business Logic
- **Unlocks:** S-01, S-02, S-03, simulator-to-dashboard verification path
- **Prerequisites:** —
- **Parallel with:** F-01
- **Blockers:** —
- **Unknowns:** —
- **Risk:** Sequenced before user-facing slices because the simulator is crucial and the current payload cannot express dead/manual-pickup or jammed decisions.
- **Status:** ready

### F-03: Minimal CI verification

- **Outcome:** (foundation) backend, simulator, and frontend verification commands run in CI.
- **Change ID:** minimal-ci-verification
- **PRD refs:** NFR: automated verification
- **Unlocks:** automated verification path for S-01, S-02, S-03, S-04, S-05, S-06
- **Prerequisites:** —
- **Parallel with:** F-04, S-01
- **Blockers:** —
- **Unknowns:** —
- **Risk:** Kept minimal so CI protects the MVP without turning into deployment automation or infrastructure work.
- **Status:** ready

### F-04: Basic operational visibility

- **Outcome:** (foundation) MVP runtime has clear logs, API errors, and dashboard runtime states for troubleshooting.
- **Change ID:** basic-operational-visibility
- **PRD refs:** NFR: basic operational visibility
- **Unlocks:** troubleshooting path for S-01, S-02, S-03, S-04, S-05, S-06
- **Prerequisites:** —
- **Parallel with:** F-03, S-01
- **Blockers:** —
- **Unknowns:** —
- **Risk:** Limited to practical logs and runtime states so the MVP gains debuggability without pulling in a metrics, tracing, or error-tracking platform.
- **Status:** ready

## Slices

### S-01: Live fleet overview

- **Outcome:** user can view a protected fleet overview fed by simulator telemetry.
- **Change ID:** live-fleet-overview
- **PRD refs:** US-01, FR-002, FR-003, FR-007
- **Prerequisites:** F-01, F-02
- **Parallel with:** —
- **Blockers:** —
- **Unknowns:** —
- **Risk:** This is the fastest visible proof after the foundations; if overview status is unclear, later details and actions inherit that confusion.
- **Status:** proposed

### S-02: Vehicle health details

- **Outcome:** user can open selected vehicle details that preserve healthy, unhealthy, dead, and jammed distinctions.
- **Change ID:** vehicle-health-details
- **PRD refs:** US-01, FR-003, FR-004, FR-006
- **Prerequisites:** S-01
- **Parallel with:** —
- **Blockers:** —
- **Unknowns:** —
- **Risk:** Details are sequenced before actions to reduce wrong-vehicle and wrong-state actions, matching the PRD guardrails.
- **Status:** proposed

### S-03: Safe vehicle action

- **Outcome:** user can recall a movable unhealthy vehicle or see dead and jammed vehicles marked for manual pickup.
- **Change ID:** safe-vehicle-action
- **PRD refs:** US-01, FR-005, FR-006
- **Prerequisites:** S-02
- **Parallel with:** —
- **Blockers:** —
- **Unknowns:** —
- **Risk:** This introduces the highest consequence behavior, so it depends on selected-vehicle details and explicit state distinctions.
- **Status:** proposed

### S-04: Logged-in intervention flow

- **Outcome:** user can complete the logged-in unhealthy/dead vehicle intervention flow and see status update.
- **Change ID:** logged-in-intervention-flow
- **PRD refs:** US-01, FR-001, FR-002, FR-003, FR-004, FR-005, FR-006, FR-007
- **Prerequisites:** S-03
- **Parallel with:** —
- **Blockers:** —
- **Unknowns:** —
- **Risk:** This packages the north star into one verifiable flow; deferring it longer would optimize pieces without proving the MVP works end to end.
- **Status:** proposed

### S-05: Manager account creation

- **Outcome:** user can create another fleet manager from the protected dashboard.
- **Change ID:** manager-account-creation
- **PRD refs:** US-02, FR-001, FR-008, Access Control
- **Prerequisites:** S-04
- **Parallel with:** S-06
- **Blockers:** —
- **Unknowns:** —
- **Risk:** Sequenced after the north star because account administration is useful, but it should not delay proving that the logged-in intervention flow works.
- **Status:** proposed

### S-06: Simulated vehicle creation

- **Outcome:** user can create a simulated vehicle from the protected dashboard and see it enter fleet monitoring.
- **Change ID:** simulated-vehicle-creation
- **PRD refs:** US-03, FR-002, FR-007, FR-009
- **Prerequisites:** S-04
- **Parallel with:** S-05
- **Blockers:** —
- **Unknowns:** —
- **Risk:** Kept after the north star and limited to simulated vehicle records so dashboard fleet setup does not drift into physical vehicle integration.
- **Status:** proposed

## Backlog Handoff

| Roadmap ID | Change ID | Suggested issue title | Ready for `/10x-plan` | Notes |
|---|---|---|---|---|
| F-01 | minimal-login-gate | Enforce minimal fleet-manager login gate | yes | Run `/10x-plan minimal-login-gate`; this unlocks protected dashboard work. |
| F-02 | health-telemetry-contract | Align simulator and dashboard on health-state telemetry | yes | Run `/10x-plan health-telemetry-contract`; this unlocks all health-state slices. |
| F-03 | minimal-ci-verification | Add minimal CI verification for backend, simulator, and frontend | yes | Can run in parallel with S-01; keep auto-deploy out of scope. |
| F-04 | basic-operational-visibility | Add basic MVP logs, API errors, and dashboard runtime states | yes | Can run in parallel with S-01; keep metrics/tracing/error tracking out of scope. |
| S-01 | live-fleet-overview | Show live protected fleet overview from simulator telemetry | no | Needs F-01 and F-02 first. |
| S-02 | vehicle-health-details | Add selected vehicle health details | no | Needs S-01 first. |
| S-03 | safe-vehicle-action | Add safe recall and manual-pickup actions | no | Needs S-02 first. |
| S-04 | logged-in-intervention-flow | Verify the complete logged-in intervention flow | no | Needs S-03 first; this is the north-star validation flow. |
| S-05 | manager-account-creation | Add protected fleet-manager account creation | no | Needs S-04 first; this extends dashboard administration after the north star is proven. |
| S-06 | simulated-vehicle-creation | Add protected simulated vehicle creation | no | Needs S-04 first; this stays limited to simulated fleet records. |

## Open Roadmap Questions

None.

## Parked

- **Fleet mechanic profiles** — Why parked: PRD Non-Goals exclude mechanic profiles; the MVP serves fleet managers only.
- **Battery usage history analysis** — Why parked: PRD Non-Goals focus the MVP on current vehicle state.
- **Failure history analysis** — Why parked: PRD Non-Goals exclude historical diagnostics.
- **Historical vehicle status overview** — Why parked: PRD Non-Goals keep the dashboard focused on current fleet condition.
- **Severity ranking** — Why parked: PRD Non-Goals identify severity ranking as a future-scale concern.
- **Physical vehicle integration** — Why parked: PRD Non-Goals specify simulated fleet telemetry for MVP.
- **Role-based manager administration** — Why parked: PRD Access Control keeps a flat fleet-manager permission model for MVP.
- **Mobile dashboard support** — Why parked: PRD Non-Goals target desktop browsers only.
- **Auto-deploy pipeline and infrastructure-as-code** — Why parked: PRD Non-Goals keep MVP automation to verification checks only.
- **Deep observability stack** — Why parked: PRD Non-Goals keep MVP visibility to logs, clear API errors, and dashboard runtime states.

## Done
