# Frame Brief: Health telemetry contract

> Framing step before /10x-plan. This document captures what is *actually*
> at issue, separated from what was initially assumed.

## Reported Observation

F-02 says the current telemetry payload is not the PRD health-state contract
and cannot express healthy, unhealthy, dead, or jammed decisions.

## Initial Framing (preserved)

- **User's stated cause or approach**: Treat this as a cross-system telemetry contract problem across simulator, backend, persistence, and frontend.
- **User's proposed direction**: Frame `health-telemetry-contract` before planning it.
- **Pre-dispatch narrowing**: User confirmed the framing is okay; treat this as one focused foundation concern, not several separate observations. User added that `lap` should not remain because the vehicle is not racing; future RC vehicles will scout and analyze an area.

## Dimension Map

The observation could originate at any of these dimensions:

1. **Product rule definition** — the PRD might not actually require a structured health-state contract, making F-02 over-scoped.
2. **Telemetry generation** — the simulator may be the only layer still emitting the wrong facts.
3. **Backend API and persistence contract** — the backend may preserve the old wire/storage shape, preventing health-state data from surviving ingestion and retrieval. <- initial framing
4. **Frontend interpretation** — the frontend may receive enough data but render it with race/pit language.
5. **Slice boundary** — health contract work may belong in later overview/details/action slices instead of this foundation.

## Hypothesis Investigation

| Hypothesis | Evidence | Verdict |
| --- | --- | --- |
| Product rule definition: no explicit contract needed | PRD requires healthy/unhealthy/dead distinctions, lost-connection dead marking, jammed manual pickup, and a rule consuming battery level, heartbeat, motor temperature, speed mismatch, and jam detection. See `context/foundation/prd.md:57`, `context/foundation/prd.md:60`, `context/foundation/prd.md:61`, `context/foundation/prd.md:92`, `context/foundation/prd.md:94`. | NONE |
| Telemetry generation: simulator-only issue | Simulator emits `lap`, `tire_wear`, temperatures, `speed_kph`, and `pit_recommended`; it has no heartbeat, speed mismatch, jam detection, health state, reason, or pickup/recall decision fields. User also confirmed `lap` is domain-wrong because vehicles scout/analyze areas rather than race laps. See `simulator/src/main.rs:8`, `simulator/src/main.rs:71`. | WEAK |
| Backend API and persistence contract: old contract is preserved end to end | Backend `TelemetryReading` has the old race/pit fields, the startup table stores those fields, and load/insert paths select and bind those exact columns. See `backend/src/main.rs:60`, `backend/src/main.rs:315`, `backend/src/main.rs:433`, `backend/src/main.rs:462`. | STRONG |
| Frontend interpretation: UI-only mismatch | Frontend type and dashboard metrics consume the old fields and render lap, speed, pit alerts, tire wear, and Pit/Stay out. It cannot render dead/manual-pickup or jammed distinctions from the current type. See `frontend/src/App.tsx:7`, `frontend/src/App.tsx:185`, `frontend/src/App.tsx:275`, `frontend/src/App.tsx:291`. | STRONG |
| Slice boundary: defer health distinctions to later slices | Roadmap explicitly makes F-02 the foundation that unlocks S-01, S-02, S-03, and S-04, because the current payload cannot express dead/manual-pickup or jammed decisions. See `context/foundation/roadmap.md:74`, `context/foundation/roadmap.md:79`, `context/foundation/roadmap.md:84`. | NONE |

## Narrowing Signals

Step 3 found strong evidence for the cross-system contract hypothesis and no
credible evidence that this is only a UI wording issue, only simulator sample
data, or a later-slice concern. Skipping additional questioning; reframing
directly would add ceremony without changing the conclusion.

- The PRD and shape notes both make health-state output part of the domain rule, not an optional dashboard embellishment.
- The simulator, backend struct, database schema, API retrieval path, and frontend type all carry the old race/pit payload.
- User confirmed `lap` is not a valid domain concept for the target vehicle behavior; the contract should replace race progress with scouting/area-oriented telemetry if progress/context is still needed.
- Later slices depend on preserving the health-state distinctions; they should not invent the contract independently.

## Cross-System Convention

This class of observation is usually handled by defining a single explicit
domain/wire contract first, then making each producer, persistence path, API
response, and consumer preserve that contract. That convention matches the
repo's own hard rule in `AGENTS.md`: backend and simulator telemetry structs
duplicate the wire contract, so both must change together and the frontend
`TelemetryReading` type must be verified.

## Reframed (or Confirmed) Problem Statement

> **The actual problem to plan around is**: The simulator-to-dashboard telemetry contract still models race pit decisions instead of MVP vehicle health decisions, so health state, reasons, and action eligibility cannot survive the end-to-end data path.

The initial framing was correct. Planning should focus on aligning the
contract across simulator publishing, backend ingestion/storage/API return,
and frontend consumption, while staying below the later feature work for
selected details and recall actions.

## Confidence

- **HIGH** — strong evidence + matches convention + decisive narrowing signal

The evidence appears in every contract surface that currently carries
telemetry, and the roadmap explicitly positions this as the foundation before
the user-facing health slices.

## What Changes for /10x-plan

Plan F-02 as an end-to-end contract alignment change, not as a dashboard polish
task. The plan should define the health-state payload and minimum derived
fields needed by later slices, explicitly removing race-only concepts such as
`lap`. If the dashboard still needs mission context, replace it with a
scouting/area-oriented concept rather than renaming it mechanically. Then
update simulator, backend persistence/API, and frontend types/rendering enough
to prove healthy, unhealthy, dead, and jammed distinctions survive the system.

## References

- Source files: `context/foundation/prd.md:57`, `context/foundation/prd.md:92`, `context/foundation/prd.md:94`, `context/foundation/roadmap.md:74`, `simulator/src/main.rs:8`, `backend/src/main.rs:60`, `backend/src/main.rs:315`, `frontend/src/App.tsx:7`
- Related research: none present at `context/changes/health-telemetry-contract/research.md`
- Investigation tasks: local investigation only; sub-agent spawning was unavailable because this session's delegation tool requires explicit user authorization for sub-agents.
