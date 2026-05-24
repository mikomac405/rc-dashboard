---
project: RC Smart Pit-Stop
researched_at: 2026-05-24T23:32:22+02:00
recommended_platform: Railway
runner_up: Fly.io
context_type: mvp
tech_stack:
  language: Rust + TypeScript
  framework: Axum API + Vite React frontend
  runtime: Dockerized services with Postgres 17, Mosquitto MQTT, and Rust simulator
---

## Recommendation

**Deploy on Railway.**

Railway is the best fit for this MVP because the top priority is to keep the Docker Compose mental model and avoid managing infrastructure directly. It will not run `docker-compose.yml` as a single production artifact, but it maps the current shape most naturally: backend API, frontend, simulator, Mosquitto, and Postgres become Railway services in one project, connected by private networking and environment variables.

Fly.io remains the runner-up for a more Rust-native, professional container platform, and a small Linux VPS would likely be cheapest. Railway wins because the goal is "build the Dockerized app and worry about as little else as possible," while still keeping a CLI, managed deploys, logs, database templates, private networking, and agent-friendly operations.

Primary sources checked on 2026-05-24: [Railway CLI](https://docs.railway.com/cli), [Railway deploy templates](https://docs.railway.com/cli/deploy), [Railway deployment actions](https://docs.railway.com/deployments/deployment-actions), [Railway pricing FAQ](https://docs.railway.com/pricing/faqs), [Railway private networking](https://docs.railway.com/private-networking), [Fly Launch](https://fly.io/docs/launch/), [Rust on Fly.io](https://fly.io/docs/rust/), [Fly pricing](https://fly.io/docs/about/pricing/), [Render CLI](https://render.com/docs/cli), [Render Docker](https://render.com/docs/docker), and [Render MCP](https://render.com/docs/mcp-server).

## Platform Comparison

| Platform | CLI-first | Managed/Serverless | Agent-readable docs | Stable deploy API | MCP / Integration | Total | Fit |
|---|---|---|---|---|---|---|---|
| Cloudflare Workers + Pages | Pass | Partial | Pass | Pass | Pass | 4.0 / 5 | Filtered: awkward primary fit for Axum + MQTT + Postgres |
| Vercel | Pass | Pass | Pass | Pass | Partial | 4.5 / 5 | Filtered: serverless functions do not fit persistent MQTT/simulator needs |
| Netlify | Pass | Pass | Pass | Partial | Pass | 4.0 / 5 | Filtered: no always-on Rust/Axum container host for this backend |
| Railway | Pass | Pass | Pass | Partial | Pass | 4.0 / 5 | Recommended; closest managed fit for the current multi-service Docker shape |
| Fly.io | Pass | Partial | Pass | Pass | Partial | 4.0 / 5 | Runner-up; stronger Rust/container platform, more operational responsibility |
| Render | Pass | Pass | Pass | Pass | Pass | 5.0 / 5 | Third; strong platform, likely higher cost for several always-on services |

Cloudflare is excellent for static frontend and edge functions. It now has container capabilities, but this project is a Docker Compose-style Rust API, MQTT broker, Postgres database, and simulator. Cloudflare would add platform-specific adaptation without solving co-located Postgres cleanly.

Vercel is attractive for Vite/static hosting and has strong CLI and rollback ergonomics. It is not a good primary home for this MVP because the backend is not a collection of request/response functions; it needs persistent MQTT/background process semantics.

Netlify has excellent docs, CLI, static hosting, and MCP support. It is filtered out for the same reason as Vercel: background functions are bounded jobs, not a durable Rust API plus MQTT broker and simulator runtime.

Railway is the simplest full-stack PaaS match for the current Compose shape. It supports multiple services, private networking, Postgres templates, logs, and an agent-friendly CLI. Its main caveat is that service rollback is documented mostly through deployment UI actions, and usage billing can exceed the $5 Hobby floor if several services stay online.

Fly.io supports Rust, Docker, private networking, process groups, logs, and deployment through `flyctl`. It is a strong runner-up, but it asks for more platform-specific infrastructure decisions around apps, machines, process groups, volumes, autostop, and database choice.

Render is a very strong traditional PaaS option with Docker, Rust, background workers, Postgres, CLI, API, and official MCP support. It ranks third for this MVP because always-on API, simulator, broker, frontend, and Postgres services are likely to exceed the desired low-cost profile faster than Railway.

### Shortlisted Platforms

#### 1. Railway (Recommended)

Railway won because it best matches the user's desired operating model: build the Dockerized services, keep them together in one managed project, and avoid server administration. It provides the most natural path from local Compose to production PaaS without moving to raw VPS operations.

#### 2. Fly.io

Fly.io is the stronger Rust/container learning platform and still a good choice if the project needs finer control. It loses here because the user values low operational burden more than platform-control practice.

#### 3. Render

Render is the most complete PaaS on paper: Docker, Rust, Postgres, workers, CLI, and MCP all score well. It is held back by expected always-on service cost for this small MVP.

## Anti-Bias Cross-Check: Railway

### Devil's Advocate - Weaknesses

1. Railway can make the first deploy feel deceptively simple, but this repo is five services, not one. The agent needs a deliberate service and variable map instead of a blind `railway up`.
2. Mosquitto is not a first-class managed Railway primitive. It likely runs as a Docker service, so broker persistence, auth, restart behavior, and networking remain project responsibilities.
3. Railway's low starting price is attractive, but multiple always-on services plus Postgres can exceed the expected monthly floor because RAM, CPU, storage, and egress are usage-billed.
4. Rollbacks exist, but database/schema changes and telemetry contract changes do not roll back automatically.
5. Railway convenience can hide production boundaries: service tokens, public/private networking, environment variables, database exposure, and agent permissions still need explicit review.

### Pre-Mortem - How This Could Fail

Six months after launch, Railway turned painful because the team treated the Docker Compose file as if it mapped perfectly to production. The API, simulator, Mosquitto, and Postgres all deployed, but service variables were patched manually and never captured cleanly in config. A simulator loop consumed more memory than expected, the MQTT broker restarted during deploys, and telemetry gaps looked like dead vehicles in the dashboard. Costs stayed small at first, then crept beyond the expected baseline as always-on RAM and Postgres storage accumulated. During a bad release, the app rollback worked, but the data model change did not, forcing manual repair. The platform was not the failure; the failure was assuming a low-ops PaaS removed the need to define service boundaries, networking, secrets, and rollback policy.

### Unknown Unknowns

- Railway is friendly to Docker, but Docker Compose itself is not the deploy artifact; services need to be modeled individually.
- MQTT should stay on private service-to-service networking at first. Public TCP exposure adds security and routing decisions.
- Postgres templates are convenient, but backup/restore, version posture, and retention policy still need explicit confirmation.
- Low traffic does not mean low cost if several services are always running.
- Agent/MCP access should start read-only or tightly scoped until deploy operations are proven.

## Operational Story

- **Preview deploys**: Use Railway environments for staging/production. PR deploys can mirror workloads, but they can also create extra billable running services, so start with one staging environment before enabling automatic PR previews.
- **Secrets**: Store `DATABASE_URL`, `MQTT_HOST`, `MQTT_PORT`, `MQTT_TOPIC`, auth secrets, and service credentials as Railway variables. Humans set or rotate production secret values; the agent may inspect variable names and propose changes.
- **Rollback**: Railway deployment rollback restores the previous successful deployment image and custom variables when available, but database migrations and telemetry contract changes require a manual recovery plan.
- **Approval**: An agent may deploy staging, inspect logs, and propose production changes. A human approves production deploys, primary secret rotation, database replacement, public TCP exposure, and destructive service/database actions.
- **Logs**: Use `railway logs --service <service> --environment <environment>` for runtime logs, `railway logs --deployment --latest` for deployment logs, and `--json` when an agent needs structured output.

## Risk Register

| Risk | Source | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| Compose file does not map directly to Railway deploys | Devil's advocate | High | High | Create an explicit Railway service map for backend, frontend, simulator, Mosquitto, and Postgres before first deploy. |
| Mosquitto is self-managed on Railway | Devil's advocate | Medium | High | Keep MQTT private at first, define broker credentials, and document restart/persistence behavior before exposing TCP publicly. |
| Usage-based cost exceeds expectation | Devil's advocate | Medium | Medium | Start with minimum service sizes, disable unnecessary preview environments, and check Railway usage after the first week. |
| Rollback restores app deploy but not data state | Pre-mortem | Medium | High | Add a migration policy before schema changes: backup first, forward-only migrations, and manual rollback notes. |
| Service variables drift from local Compose | Pre-mortem | Medium | High | Maintain a checked-in service/variable map derived from `.env.example` and `docker-compose.yml`. |
| Telemetry gaps are misread as vehicle failure | Pre-mortem | Medium | Medium | Add health checks and logs for simulator and broker before relying on dashboard state in staging. |
| Agent receives too much production access | Unknown unknowns | Medium | High | Use scoped Railway tokens, prefer staging deploys for agent execution, and keep destructive production operations human-only. |
| PR preview environments increase spend | Research finding | Medium | Medium | Keep automatic PR deploys disabled until the baseline production cost is known. |

## Getting Started

1. Install and authenticate the Railway CLI, then create a Railway project named `rc-smart-pit-stop`.
2. Deploy Postgres from a Railway template with `railway deploy --template postgres`, then capture the generated connection variable.
3. Create separate Railway services for `backend`, `frontend`, `simulator`, and `mqtt`; use the existing Dockerfiles where possible.
4. Translate `docker-compose.yml` service names into Railway private-network hostnames and set `DATABASE_URL`, `MQTT_HOST`, `MQTT_PORT`, and `MQTT_TOPIC` as Railway variables.
5. Deploy staging first, then verify `/health`, `/api/telemetry`, simulator publishing, Mosquitto connectivity, and logs with `railway logs --service <service> --environment staging`.

## Out of Scope

The following were not evaluated in this research:

- Docker image configuration
- CI/CD pipeline setup
- Production-scale architecture (multi-region, HA, DR)
- Full VPS self-hosting plan
