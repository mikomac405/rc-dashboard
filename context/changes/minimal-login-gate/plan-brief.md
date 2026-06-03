# Minimal Login Gate — Plan Brief

> Full plan: `context/changes/minimal-login-gate/plan.md`

## What & Why

Build the MVP login gate that lets a fleet manager authenticate before seeing dashboard telemetry. This satisfies FR-001 and creates the protected-access foundation needed before the live fleet overview and later recall paths.

## Starting Point

The backend currently exposes telemetry publicly and the frontend polls `/api/telemetry` without credentials. Auth is absent from code, environment defaults, Compose config, and frontend state.

## Desired End State

On first run, the backend seeds a flat fleet-manager account with `admin:admin` and forces a password change before telemetry access. After the password is changed, the frontend uses an 8-hour signed bearer token to access protected telemetry, and invalid sessions return the user to login.

## Key Decisions Made

| Decision | Choice | Why (1 sentence) | Source |
| --- | --- | --- | --- |
| Access scope | Login, logout, protected dashboard/API | Completes the MVP login requirement without adding roles or account-management extras. | Plan |
| Credential model | Database-stored first-run `admin:admin` with forced change | Gives the MVP a real credential lifecycle while preserving the flat fleet-manager model. | Plan |
| Password storage | Argon2 hashes in Postgres | Passwords should not be stored plainly or with weak hashes, even for an MVP. | Plan |
| Session model | Signed 8-hour bearer token | Fits the SPA/proxy shape cleanly and avoids cookie/CORS complexity. | Plan |
| Route boundary | Protect dashboard telemetry reads; keep health and simulator ingest separate | Protects user data now without confusing fleet-manager auth with service ingestion auth. | Plan |
| DB fallback | Auth requires Postgres | Forced password-change state must be durable and should not split into an in-memory auth mode. | Plan |
| Verification | Backend auth tests plus frontend build/lint/manual flow | Covers risky auth behavior without introducing a frontend test framework yet. | Plan |

## Scope

**In scope:**

- Fleet-manager login and logout.
- First-run `admin:admin` bootstrap when no users exist.
- Forced password change before dashboard access.
- Argon2 password hashes stored in Postgres.
- Signed bearer tokens with 8-hour expiry.
- Protected `GET /api/telemetry`.
- Frontend login/password-change/authenticated dashboard states.
- Auth env/default updates and backend auth tests.

**Out of scope:**

- Roles, mechanic profiles, invites, password reset, OAuth/OIDC, and audit logs.
- Service-token protection for simulator ingest.
- Telemetry payload changes.
- New frontend test or e2e framework.

## Architecture / Approach

Axum remains the auth authority. Startup creates the auth table and seeds `admin` only when the user table is empty. Login either returns a limited password-change token or a normal fleet-manager token; protected telemetry accepts only normal unexpired bearer tokens. React stores the normal token for the MVP, sends it on telemetry fetches, and clears it on logout or `401`.

## Phases at a Glance

| Phase | What it delivers | Key risk |
| --- | --- | --- |
| 1. Backend Auth Foundation | Durable credential, forced-change flow, token validation, protected telemetry route | Getting auth unavailable behavior and limited-token boundaries wrong |
| 2. Frontend Login Gate | Login/password-change/logout UI and authenticated telemetry polling | Accidentally showing stale telemetry or conflating offline with logged-out |
| 3. Config, Verification, and Documentation Alignment | Env/Compose updates plus full verification loop | Missing a deployment/local secret default or skipping manual flow checks |

**Prerequisites:** `context/changes/minimal-login-gate/change.md` exists; Postgres is available for auth verification.
**Estimated effort:** ~2-3 focused sessions across 3 phases.

## Open Risks & Assumptions

- Existing inline startup migrations remain acceptable for this MVP; file migrations are not introduced here.
- Browser token storage is acceptable for the MVP, with an 8-hour expiry and logout clearing local state.
- The first implementation may need dependency choices for token signing and Axum route tests, but the plan constrains their behavior rather than prescribing exact crates.

## Success Criteria (Summary)

- A fleet manager cannot see telemetry until logged in and past the forced password-change flow.
- `GET /api/telemetry` returns `401` without a valid normal bearer token while `/health` remains public.
- Backend auth tests pass, and the frontend build/lint plus manual browser flow confirm login, password change, logout, and invalid-token recovery.
