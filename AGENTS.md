# Repository Guidelines

RC Smart Pit-Stop is a self-hosted dashboard MVP with a Rust Axum API, Rust MQTT telemetry simulator, and Vite React TypeScript frontend. Use @context/foundation/prd.md and @context/foundation/tech-stack.md as product and stack sources of truth.

## Hard Rules

- Do not write under `context/archive/`; archived changes are read-only by convention.
- Keep the health telemetry contract aligned across @backend/src/main.rs, @simulator/src/main.rs, and @frontend/src/App.tsx before changing API shape.
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
- Backend and simulator telemetry structs duplicate the health-state wire contract; update both together and verify the frontend `TelemetryReading` type.
- Frontend TypeScript uses React function components, hooks, and CSS files beside components as shown in @frontend/src/App.tsx and @frontend/src/App.css.

## Testing and CI

- No automated tests or GitHub Actions exist yet. Use `cargo build` for Rust crates, `npm run build`, and `npm run lint` for the frontend before handoff.
- When adding tests, place them in the package they verify and add the exact run command here once it exists.

## Git and Review Notes

- This repository does not establish a separate commit-message or PR convention beyond the 10x phase and epilogue commit ritual.
- Keep generated build output out of reviews unless the task explicitly asks for it; `frontend/dist/`, `frontend/node_modules/`, and Rust `target/` directories are local artifacts.

<!-- BEGIN @przeprogramowani/10x-cli -->

## 10xDevs AI Toolkit - Module 2, Lesson 4

Prepare for a harder implementation stream with the **research-backed planning chain**:

```
internal research (/10x-research) + external research (exa.ai, Context7) -> /10x-plan -> /10x-implement -> success
```

The lesson focus is distinguishing internal from external research and using evidence to back planning decisions.

### Task Router - Where to start

| Skill | Use it when |
| --- | --- |
| **Internal research (lesson focus)** | |
| `/10x-research <change-id>` | You need evidence from the existing codebase — patterns, conventions, integration points, or existing implementations. Runs parallel sub-agents over the repo and writes structured findings to `research.md`. |
| **External research (lesson focus)** | |
| exa.ai | You need AI-native web search for library comparisons, best practices, or ecosystem context that the codebase cannot answer. |
| Context7 (`resolve-library-id` → `get-library-docs`) | You need live, current documentation for a specific library or framework. Resolves a library ID first, then fetches relevant doc pages. |
| **Framing spare wheel** | |
| `/10x-frame <change-id>` | The plan won't converge, the plan doesn't deliver expected results, or persistent drift keeps breaking the implementation. Use as an escape hatch on a separate problem (demonstrated on Space Explorers example), not as pre-research ritual. |
| **Planning and execution** | |
| `/10x-plan <change-id>` / `/10x-implement <change-id> phase <n>` | Use the same planning and execution chain from Lesson 2, now with upstream research evidence feeding the plan. |

### Research discipline

- Internal research (`/10x-research`) answers "what does our codebase already do?" — patterns, schemas, conventions, integration points.
- External research (exa.ai, Context7) answers "what should we do?" — library capabilities, API docs, ecosystem best practices.
- Combine both as evidence-backed input to `/10x-plan`. A plan without research evidence on a non-trivial stream is a guess.
- Agent-friendly docs (`llms.txt`, markdown-for-agents, `/md` endpoints) are a quality signal for library selection — libraries that publish agent-readable docs integrate faster.

### `/10x-frame` as spare wheel

Three triggers for reaching for `/10x-frame`:
1. The plan won't converge — research keeps opening more questions instead of narrowing to a contract.
2. The plan doesn't deliver — implementation repeatedly fails to meet success criteria.
3. Persistent drift — the implementation keeps diverging from the plan in ways that suggest the problem was mis-framed.

Demonstrated on a Space Explorers example, not the SRS path. It is an escape hatch, not a mandatory step.

### Paths used by this lesson

- `context/changes/<change-id>/research.md` - internal research output
- `context/changes/<change-id>/frame.md` - framing output when needed
- `context/changes/<change-id>/plan.md` - evidence-backed implementation contract
- `context/foundation/lessons.md` - recurring rules and pitfalls

Skills must not write to `context/archive/`. Archived changes are immutable; if a resolved target path starts with `context/archive/`, abort with: "This change is archived. Open a new change with `/10x-new` instead."

<!-- END @przeprogramowani/10x-cli -->
