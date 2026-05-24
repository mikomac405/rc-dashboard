---
project: "RC Smart Pit-Stop"
version: 1
status: draft
created: 2026-05-23
context_type: greenfield
product_type: web-app
target_scale:
  users: small
  qps: low
  data_volume: small
timeline_budget:
  mvp_weeks: 3
  hard_deadline: 2026-06-28
  after_hours_only: true
---

# RC Smart Pit-Stop PRD

## Vision & Problem Statement

Fleet managers lose time and fleet reliability when they must manually analyze autonomous RC vehicle data and mechanical condition to detect broken or dead vehicles.

RC Smart Pit-Stop starts with a simulated fleet that sends data to a dashboard, so the fleet manager can learn which telemetry and control capabilities need to be added to future physical vehicles for reliable fleet operation. At 100x scale, the domain rule would need severity ranking.

## User & Persona

The primary persona for the MVP is a fleet manager responsible for monitoring autonomous RC vehicle fleet health and detecting broken or dead vehicles.

The fleet manager reaches for the product when they need to understand which vehicles are unhealthy, dead, or require intervention without manually inspecting each vehicle.

## Success Criteria

### Primary

- A fleet manager logs in, sees the fleet overview, notices an unhealthy vehicle or an automatically marked dead vehicle, opens vehicle details, recalls a movable unhealthy vehicle to pit-stop or sees a dead or jammed unhealthy vehicle marked for manual pickup, and sees the vehicle status update.

### Secondary

- Dead or broken vehicles are charged or fixed within 10 minutes after manual collection or a visit to pit-stop.

### Guardrails

- Dead and unhealthy vehicle states cannot be confused.
- The dashboard must not recall the wrong vehicle.

## User Stories

### US-01: Fleet manager handles an unhealthy or dead vehicle

- **Given** a logged-in fleet manager viewing a simulated fleet
- **When** the dashboard shows an unhealthy or dead vehicle
- **Then** the fleet manager can open its details, recall a movable unhealthy vehicle to pit-stop or see a dead or jammed unhealthy vehicle already marked for manual pickup, and see the vehicle status update

#### Acceptance Criteria

- The fleet overview shows enough status information for the fleet manager to identify unhealthy and dead vehicles.
- The vehicle details view preserves the distinction between unhealthy vehicles and dead vehicles.
- Recall actions update the vehicle status after the fleet manager applies them to movable unhealthy vehicles.
- A vehicle that loses connection is automatically marked as dead for manual pickup.
- A connected vehicle that cannot move is treated as an unhealthy vehicle with a jammed reason and is marked for manual pickup instead of recall.
- The dashboard does not apply a recall or manual-pickup action to a different vehicle than the one selected.

## Functional Requirements

- FR-001: Fleet manager can log in. Priority: must-have
  > Socrates: No counter-argument; it stands as written.
- FR-002: Fleet manager can view fleet overview. Priority: must-have
  > Socrates: Counter-argument considered: "Vehicle details alone might be enough for a tiny simulated fleet." Resolution: kept; overview is essential because fleet detection starts from scanning fleet health.
- FR-003: Fleet manager can distinguish healthy, unhealthy, and dead vehicles. Priority: must-have
  > Socrates: Counter-argument considered: "Three states may be too coarse if the real diagnostic model needs richer states." Resolution: kept; three states are enough because the MVP decision is recall vs. manual pickup.
- FR-004: Fleet manager can open vehicle details. Priority: must-have
  > Socrates: Counter-argument considered: "Details view may be unnecessary if the overview shows all decision data." Resolution: kept; details view prevents wrong actions by showing the selected vehicle context.
- FR-005: Fleet manager can recall a movable unhealthy vehicle to pit-stop. Priority: must-have
  > Socrates: Counter-argument considered: "Simulated recall might fake control before the physical vehicle supports it." Resolution: kept; simulated recall reveals the control contract physical vehicles need.
- FR-006: Fleet manager can see a lost-connection vehicle automatically marked as dead for manual pickup. Priority: must-have
  > Socrates: Counter-argument considered: "Manual pickup marking might be operational tracking rather than smart pit-stop logic." Resolution: revised; if a vehicle is dead because it lost connection, it should be automatically marked as dead for manual pickup.
- FR-007: Fleet manager can see vehicle status updates from simulated fleet telemetry. Priority: must-have
  > Socrates: Counter-argument considered: "Fleet telemetry simulation may distract from dashboard UX if data generation becomes too complex." Resolution: kept; Fleet telemetry simulation is necessary because the MVP exists to shape vehicle telemetry and control needs.

## Non-Functional Requirements

- Fleet managers see telemetry-derived vehicle state changes at a 30-second refresh cadence.
- Dead vehicles, movable unhealthy vehicles, and jammed unhealthy vehicles are visibly distinct before the fleet manager can take an action.
- Only logged-in fleet managers can see telemetry and issue recalls.
- The MVP dashboard works on desktop browsers only.

## Business Logic

RC Smart Pit-Stop analyzes real-time telemetry to autonomously diagnose mechanical anomalies and flag vehicles as unhealthy, so the fleet manager can decide whether to recall them.

The MVP rule consumes battery level, heartbeat, motor temperature, speed mismatch, and jam-detection telemetry.

The rule outputs `healthy`, `unhealthy`, or `dead`. An `unhealthy` state includes a reason. A jammed vehicle is connected but cannot move, so it remains unhealthy with a jammed reason and is marked for manual pickup instead of recall.

The fleet manager encounters the rule output as status badges in the fleet overview and vehicle details view.

## Access Control

Fleet managers log in to the dashboard.

The MVP uses a flat user model: every logged-in fleet manager has the same permissions.

Mechanic profiles are not included in the MVP.

## Non-Goals

- No fleet mechanic profiles in MVP; the first version serves the fleet manager persona only.
- No battery usage history analysis; the MVP focuses on current vehicle state.
- No failure history analysis; the MVP diagnoses current telemetry rather than historical patterns.
- No historical vehicle status overview; the dashboard presents current fleet condition.
- No severity ranking in MVP; severity ranking is a future-scale concern for larger fleets.
- No physical vehicle integration yet; Fleet telemetry data is simulated.
- No mobile dashboard support; the MVP targets desktop browsers only.

## Open Questions

No open questions captured during shaping.
