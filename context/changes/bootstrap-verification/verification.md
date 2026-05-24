---
bootstrapped_at: 2026-05-24T13:54:15Z
starter_id: axum
starter_name: Axum (Rust web framework)
project_name: rc-smart-pit-stop
language_family: rust
package_manager: cargo
cwd_strategy: subdir-then-move
bootstrapper_confidence: first-class
phase_3_status: ok
audit_command: cargo audit
---

## Hand-off

```yaml
starter_id: axum
package_manager: cargo
project_name: rc-smart-pit-stop
hints:
  language_family: rust
  team_size: solo
  deployment_target: self-host
  ci_provider: github-actions
  ci_default_flow: auto-deploy-on-merge
  bootstrapper_confidence: first-class
  path_taken: custom
  quality_override: false
  self_check_answers:
    typed: true
    from_official_starter: true
    conventions: true
    docs_current: true
    can_judge_agent: true
  has_auth: true
  has_payments: false
  has_realtime: false
  has_ai: false
  has_background_jobs: true
```

RC Smart Pit-Stop is a small, after-hours MVP with login, simulated telemetry, and background job needs, and you chose a Rust backend plus TypeScript frontend architecture. The registry has no single full-stack Rust+TypeScript dashboard starter, so Axum is the backend starter: type-safe, mainstream within Rust web services, documented, and suitable for an authenticated API. The frontend should be added as a Vite + React + TypeScript dashboard alongside Axum. The system also needs an MQTT telemetry simulator and broker; for the MVP, Axum should own the telemetry consumer and internal scheduled state checks, with a separate worker service left as a later split if load or complexity grows. Self-host keeps Docker Compose practical for the API, frontend, database, broker, and simulator shape. GitHub Actions with auto-deploy-on-merge gives the simplest solo CI/CD path. Bootstrapper confidence is first-class, so scaffolding should mostly work but may need a few manual adjustments.

## Pre-scaffold verification

| Signal | Value | Severity | Notes |
| --- | --- | --- | --- |
| npm package | not run | n/a | non-JS starter |
| GitHub repo | not run | n/a | card docs_url is `https://docs.rs/axum/latest/axum/`, not a GitHub repository URL |

## Scaffold log

**Resolved invocation**: `cargo new .bootstrap-scaffold --bin --edition 2024 && cd .bootstrap-scaffold && cargo add axum tokio --features tokio/full`
**Strategy**: subdir-then-move
**Exit code**: 0 after retry with Cargo package name override
**Files moved**: 3 backend entries (`backend/Cargo.toml`, `backend/Cargo.lock`, `backend/src/`)
**Conflicts (.scaffold siblings)**: none
**.gitignore handling**: created at root for Rust and frontend build artifacts
**.bootstrap-scaffold cleanup**: deleted

Notes:
- The registry invocation first failed because Cargo rejects `.bootstrap-scaffold` as an inferred package name. The retry used `cargo new .bootstrap-scaffold --name rc_smart_pit_stop --bin --edition 2024`, then added Axum and Tokio.
- Per the user request, this run also added `frontend/` using Vite + React + TypeScript and `simulator/` as a Rust MQTT telemetry simulator.
- The Axum backend subscribes to the MQTT topic and stores the latest telemetry readings for the frontend API.
- Follow-up layout change: the Axum crate was moved from the repo root into `backend/`.
- Follow-up storage change: Postgres was added to `docker-compose.yml`, and the backend now persists telemetry readings to `telemetry_readings` when `DATABASE_URL` is reachable. If Postgres is unavailable, the backend starts in memory-only mode with a warning.
- Follow-up containerization change: Compose now builds and runs `backend`, `frontend`, and `simulator` services alongside Postgres and MQTT. Dockerfiles were added under each app directory.
- `docker-compose.yml` was added with an Eclipse Mosquitto broker service.

## Post-scaffold audit

**Tool**: `cargo audit`
**Status**: failed to run
**Reason**: local Cargo installation does not have the `cargo-audit` subcommand installed.
**Partial output (if any)**:

```text
error: no such command: `audit`

help: a command with a similar name exists: `add`

help: view all installed commands with `cargo --list`
help: find a package to install `audit` with `cargo search cargo-audit`
```

Additional verification run:
- `cargo build --manifest-path backend/Cargo.toml` passed for the Axum backend.
- `cargo build --manifest-path simulator/Cargo.toml` passed for the MQTT simulator.
- `npm run build` passed for the Vite frontend.
- `npm install` reported 0 vulnerabilities for the frontend dependency tree.
- Local smoke checks passed for `GET /health` on `127.0.0.1:3000` and the Vite page on `127.0.0.1:5173`.
- Database fallback smoke check passed: with Postgres down, the backend logged a database-unavailable warning and still served `GET /health`.
- End-to-end Postgres verification was not run in this environment because `docker` is not installed.
- Docker Compose config validation passed after containerization.
- `docker compose build` completed for backend, frontend, and simulator images.
- `docker compose up -d` started backend, frontend, simulator, Postgres, and MQTT.
- Containerized smoke checks passed: `GET /health`, frontend HTTP 200, `/api/telemetry` returning simulator readings, and Postgres row count from `telemetry_readings`.

## Hints recorded but not acted on

| Hint | Value |
| --- | --- |
| bootstrapper_confidence | first-class |
| quality_override | false |
| path_taken | custom |
| self_check_answers | typed=true, from_official_starter=true, conventions=true, docs_current=true, can_judge_agent=true |
| team_size | solo |
| deployment_target | self-host |
| ci_provider | github-actions |
| ci_default_flow | auto-deploy-on-merge |
| has_auth | true |
| has_payments | false |
| has_realtime | false |
| has_ai | false |
| has_background_jobs | true |

## Next steps

Next: a future skill will set up agent context (CLAUDE.md, AGENTS.md). For now, your project is scaffolded and verified.

Useful manual steps in the meantime:
- `git init` if you have not already to start your own repo history.
- Review any `.scaffold` siblings the conflict policy created and decide which version of each file to keep.
- Address audit findings per your project's risk tolerance once `cargo-audit` is installed.
