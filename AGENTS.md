# Repository Guidelines

RC Smart Pit-Stop is a self-hosted dashboard MVP with a Rust Axum API, Rust MQTT telemetry simulator, and Vite React TypeScript frontend. Use @context/foundation/prd.md and @context/foundation/tech-stack.md as product and stack sources of truth.

## Hard Rules

- Do not write under `context/archive/`; archived changes are read-only by convention.
- Keep telemetry payload fields aligned across @backend/src/main.rs, @simulator/src/main.rs, and @frontend/src/App.tsx before changing API shape.
- Prefer editing foundation docs in place under `context/foundation/`; change-scoped plans and research belong under `context/changes/<change-id>/`.

## Project Structure

- `backend/` is the Axum service. It exposes `/health`, `/api/telemetry`, and `/api/simulator/ingest`, consumes MQTT, and persists telemetry to Postgres with an in-memory fallback.
- `simulator/` publishes synthetic telemetry to the MQTT topic configured by `MQTT_TOPIC`.
- `frontend/` is the Vite React dashboard. Source lives in `frontend/src/`; static assets live in `frontend/public/`.
- `docker-compose.yml` wires backend, frontend, simulator, Postgres 17, and Mosquitto 2. Env defaults are mirrored in @.env.example.

## Build, Test, and Development Commands

- `docker compose up --build` from the repo root runs the full local stack.
- `cargo build` in `backend/` or `simulator/` checks the Rust services.
- `cargo run` in `backend/` starts the API on `API_HOST`/`API_PORT`; `cargo run` in `simulator/` starts MQTT publishing.
- `npm run dev -- --host 0.0.0.0` in `frontend/` runs Vite; `npm run build` runs `tsc -b` then the Vite build.
- `npm run lint` in `frontend/` runs ESLint.

## Coding Style

- Rust crates use edition 2024. Keep async entry points on Tokio and preserve env-var defaults unless @.env.example changes with them.
- Backend and simulator telemetry structs currently duplicate the wire contract; update both together and verify the frontend `TelemetryReading` type.
- Frontend TypeScript uses React function components, hooks, and CSS files beside components as shown in @frontend/src/App.tsx and @frontend/src/App.css.

## Testing and CI

- No automated tests or GitHub Actions exist yet. Use `cargo build` for Rust crates, `npm run build`, and `npm run lint` for the frontend before handoff.
- When adding tests, place them in the package they verify and add the exact run command here once it exists.

## Git and Review Notes

- This repository currently has no commits and no remote configured, so no commit-message or PR convention is established.
- Keep generated build output out of reviews unless the task explicitly asks for it; `frontend/dist/`, `frontend/node_modules/`, and Rust `target/` directories are local artifacts.

<!-- BEGIN @przeprogramowani/10x-cli -->

## 10xDevs AI Toolkit - Module 2, Lesson 2

Turn one roadmap item into the first implementation cycle with the **change planning chain**:

```
/10x-roadmap -> /10x-new -> /10x-plan -> /10x-plan-review -> /10x-implement
```

`/10x-new`, `/10x-plan`, `/10x-plan-review`, and `/10x-implement` are the lesson focus. `/10x-frame` and `/10x-research` are not required rituals here; they are escalation paths introduced in the next lesson.

### Task Router - Where to start

| Skill | Use it when |
| --- | --- |
| **Change setup (lesson focus)** | |
| `/10x-new <change-id>` | You selected a roadmap item and need a stable change folder. Creates `context/changes/<change-id>/change.md` so planning, implementation, progress, commits, and later review all share one identity. Use AFTER roadmap selection, BEFORE `/10x-plan`. |
| **Planning (lesson focus)** | |
| `/10x-plan <change-id>` | You have a change folder and need a reviewable implementation plan. Reads roadmap context, foundation docs, codebase evidence, and any existing change notes; writes `plan.md` and `plan-brief.md` with phases, file contracts, success criteria, and `## Progress`. |
| **Plan readiness (lesson focus)** | |
| `/10x-plan-review <change-id>` | You have `plan.md` and need a light pre-code readiness check. Use it to catch missing end state, weak contracts, malformed progress, scope drift, or blind spots before code changes begin. |
| **Implementation (lesson focus)** | |
| `/10x-implement <change-id> phase <n>` | You have an approved plan and want to execute one phase with verification, manual gate, commit ritual, and SHA write-back to `## Progress`. |
| **Lifecycle closure** | |
| `/10x-archive <change-id>` | A change is merged or intentionally closed. Move it out of active `context/changes/` into archive state. |

### How the chain hands off

- `/10x-new` creates the durable change identity.
- `/10x-plan` turns that identity into an implementation contract.
- `/10x-plan-review` checks the plan before the agent mutates code.
- `/10x-implement` executes one planned phase, verifies, asks for manual confirmation when needed, commits, and records progress.

### Lesson boundaries

- Plan is the default router after roadmap selection. Start with `/10x-plan` unless the problem is unclear or external evidence is blocking.
- Do not run `/10x-frame + /10x-research` as ceremony for every change.
- Do not turn this lesson into a full end-to-end product build. A checkpoint with a planned and partially or fully implemented stream is valid.
- Code review of the implemented diff belongs to Lesson 3 via `/10x-impl-review`.
- Lifecycle closure via `/10x-archive` after a change is merged or intentionally closed.

### Paths used by this lesson

- `context/foundation/roadmap.md` - upstream roadmap
- `context/changes/<change-id>/change.md` - change identity
- `context/changes/<change-id>/plan.md` - implementation contract
- `context/changes/<change-id>/plan-brief.md` - compressed handoff
- `context/foundation/lessons.md` - recurring rules and pitfalls
- `docs/reference/contract-surfaces.md` - load-bearing names registry

Skills must not write to `context/archive/`. Archived changes are immutable; if a resolved target path starts with `context/archive/`, abort with: "This change is archived. Open a new change with `/10x-new` instead."

<!-- END @przeprogramowani/10x-cli -->
