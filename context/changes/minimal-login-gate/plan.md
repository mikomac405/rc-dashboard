# Minimal Login Gate Implementation Plan

## Overview

Implement the MVP fleet-manager login gate for RC Smart Pit-Stop. The change adds a database-backed flat fleet-manager credential, first-run `admin:admin` bootstrap with forced password change, signed 8-hour bearer sessions, and protected dashboard telemetry access without introducing roles or mechanic accounts.

## Current State Analysis

The backend currently exposes `/health`, `/api/telemetry`, and `/api/simulator/ingest` without authentication. Telemetry is persisted to Postgres when available and falls back to in-memory storage if the database is unavailable. The frontend is a single React dashboard that polls `/api/telemetry` anonymously and has no login, logout, token storage, or auth-failure state.

Auth is absent even though the PRD requires that only logged-in fleet managers can see telemetry and issue recalls. The roadmap intentionally sequences this foundation before protected fleet overview and recall slices, so this plan creates the route and client patterns later changes can reuse.

## Desired End State

A fleet manager can log in with the first-run `admin:admin` credential, is forced to change that password before dashboard access, and then receives an 8-hour signed bearer token. Protected dashboard API requests send `Authorization: Bearer <token>`, and `GET /api/telemetry` rejects missing, invalid, expired, or password-change-only tokens.

The backend still serves `/health` publicly and keeps simulator/MQTT ingestion separate from fleet-manager auth. If Postgres is unavailable, health and telemetry ingestion fallback may continue, but login and protected telemetry report that auth is unavailable because credential state must remain durable.

### Key Discoveries:

- [backend/src/main.rs](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:56) defines all routes inline and currently applies only permissive CORS plus shared app state.
- [backend/src/main.rs](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:132) uses inline startup schema creation rather than file migrations, so auth schema should follow that pattern for this change.
- [frontend/src/App.tsx](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.tsx:24) fetches telemetry without credentials and treats failed fetches as `offline`.
- [.env.example](/home/mmacura/Documents/Git/rc-dashboard/.env.example:1) and [docker-compose.yml](/home/mmacura/Documents/Git/rc-dashboard/docker-compose.yml:5) have telemetry/database config but no auth signing secret yet.
- `context/foundation/prd.md` and `context/foundation/shape-notes.md` settle the access model: fleet managers log in, every logged-in fleet manager has the same permissions, and mechanic profiles are out of scope.

## What We're NOT Doing

- No role-based authorization, mechanic profiles, account management UI, invite flow, password reset, or audit log.
- No external identity provider, OAuth/OIDC, email delivery, or multi-tenant user model.
- No frontend test framework or end-to-end test framework introduction in this change.
- No service-token protection for `/api/simulator/ingest`; simulator/MQTT ingestion stays separate from fleet-manager auth for now.
- No telemetry contract changes across backend, simulator, and frontend.

## Implementation Approach

Use the existing Axum service as the auth authority. Add a small fleet-manager user table and seed a single `admin` row during startup when no users exist. Store Argon2 password hashes in Postgres, issue signed bearer tokens with a required session kind, and gate `GET /api/telemetry` behind a reusable auth extractor/helper. The frontend becomes a small auth state machine: login, required password change, authenticated dashboard, logout, and invalid-session recovery.

## Critical Implementation Details

### Auth State Durability

Postgres is required for login and protected telemetry because password hashes and forced-change state must not split between durable and in-memory modes. Existing telemetry fallback can remain, but auth handlers should return a clear unavailable response when `AppState.db` is `None`.

### Token State

The forced-password-change token must not be accepted as a normal dashboard token. The token contract needs a session kind or equivalent claim so `change password` can accept only the limited state while protected telemetry accepts only a normal fleet-manager session.

## Phase 1: Backend Auth Foundation

### Overview

Add backend credential storage, password hashing, token issuance/validation, auth routes, and telemetry route protection while preserving public health and simulator ingestion.

### Changes Required:

#### 1. Backend Dependencies

**File**: `backend/Cargo.toml`

**Intent**: Add the minimal crates needed for password hashing, signed token creation, and backend test ergonomics.

**Contract**: Include an Argon2/password-hash dependency for password verification, a signed token/JWT or HMAC-capable dependency for bearer tokens, and any test-only HTTP helper dependencies required by Axum route tests.

#### 2. Auth Data Model and Startup Seed

**File**: `backend/src/main.rs`

**Intent**: Extend the inline startup schema to create a durable fleet-manager user table and seed the first-run `admin` account only when no users exist.

**Contract**: Store username, password hash, `must_change_password`, and timestamp metadata in Postgres. The initial row is username `admin`, password `admin` hashed with Argon2, and `must_change_password = true`. Do not seed a replacement if at least one fleet-manager user already exists.

#### 3. Auth Configuration

**File**: `backend/src/main.rs`

**Intent**: Add backend configuration for token signing and session lifetime without making auth credentials hardcoded in source beyond the first-run bootstrap password.

**Contract**: Read an auth signing secret from env, default safely enough for local development, and use an 8-hour token expiry. The config must be available from `AppState` or another route-accessible structure.

#### 4. Login and Password Change Routes

**File**: `backend/src/main.rs`

**Intent**: Add API routes that authenticate the fleet manager and enforce password replacement before dashboard access.

**Contract**: Add `POST /api/auth/login`, `POST /api/auth/change-password`, and a lightweight session-check route such as `GET /api/auth/me`. Login with valid credentials and `must_change_password = true` returns a limited password-change state/token, not dashboard access. Password change requires the limited token, writes a new Argon2 hash, clears `must_change_password`, and returns a normal bearer token.

#### 5. Protected Telemetry Read

**File**: `backend/src/main.rs`

**Intent**: Require a valid normal fleet-manager bearer token for dashboard telemetry reads while leaving health and simulator ingestion behavior stable.

**Contract**: `GET /api/telemetry` requires `Authorization: Bearer <token>`. Missing, invalid, expired, or limited password-change tokens return `401 Unauthorized`. `/health`, MQTT consumption, `POST /api/telemetry`, and `POST /api/simulator/ingest` are not converted to fleet-manager login routes in this phase.

### Success Criteria:

#### Automated Verification:

- Backend compiles: `cargo build --manifest-path backend/Cargo.toml`
- Backend auth tests pass: `cargo test --manifest-path backend/Cargo.toml`
- `GET /api/telemetry` rejects requests without a normal bearer token in backend tests.
- First-run `admin:admin` login returns password-change-required state in backend tests.
- Password change clears the forced-change flag and returns a normal bearer token in backend tests.
- Auth handlers return an unavailable response when Postgres/auth storage is unavailable in backend tests or documented smoke coverage.

#### Manual Verification:

- With Postgres available, `admin:admin` can log in only far enough to change the password.
- After password change, a normal token can access `GET /api/telemetry`.
- Missing or invalid tokens cannot access `GET /api/telemetry`.
- `/health` remains publicly reachable.
- Simulator or MQTT telemetry ingestion still runs without fleet-manager credentials.

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the backend auth flow and route boundaries are correct before proceeding to the next phase.

---

## Phase 2: Frontend Login Gate

### Overview

Turn the single-screen dashboard into an authenticated app shell with login, required password change, logout, bearer-token telemetry fetches, and predictable invalid-session handling.

### Changes Required:

#### 1. Auth State and API Client

**File**: `frontend/src/App.tsx`

**Intent**: Add a small client-side auth state machine and centralize authenticated API calls.

**Contract**: Track unauthenticated, password-change-required, authenticated, loading, and auth-error states. Store the normal bearer token in browser storage for the MVP and attach it as `Authorization: Bearer <token>` to protected telemetry requests. A `401` from telemetry clears the stored token and returns the app to login.

#### 2. Login and Password Change UI

**File**: `frontend/src/App.tsx`

**Intent**: Provide the fleet manager with a direct login flow and forced first-run password replacement before telemetry appears.

**Contract**: Render a login form for username/password, then a password-change form when the backend reports that the password must change. Do not render telemetry data during the forced-change state. After successful password change, transition to the dashboard using the normal token returned by the backend.

#### 3. Dashboard Logout and Fetch Behavior

**File**: `frontend/src/App.tsx`

**Intent**: Make authenticated dashboard operation explicit and recover cleanly when the session expires.

**Contract**: Add a logout control that clears token/session state. Update telemetry polling so unauthenticated states do not call `/api/telemetry`; authenticated polling includes the bearer token and treats `401` differently from backend offline status.

#### 4. Auth Styling

**File**: `frontend/src/App.css`

**Intent**: Style login, password-change, error, and logout states using the existing restrained dashboard visual language.

**Contract**: Add responsive form, field, button, and auth-message styles without introducing a component library or changing telemetry table layout beyond authenticated shell controls.

### Success Criteria:

#### Automated Verification:

- Frontend build passes: `npm run build` in `frontend/`.
- Frontend lint passes: `npm run lint` in `frontend/`.

#### Manual Verification:

- Visiting the frontend without a token shows the login form instead of telemetry.
- Logging in with first-run `admin:admin` shows the password-change screen, not the dashboard.
- Changing the password transitions to the dashboard and telemetry loads.
- Logout clears the token and returns to login.
- Expired or invalid token behavior clears the session and returns to login.

**Implementation Note**: After completing this phase and all automated verification passes, pause here for manual confirmation from the human that the browser login and password-change flow feels correct before proceeding to the next phase.

---

## Phase 3: Config, Verification, and Documentation Alignment

### Overview

Align local/deployment configuration with the new auth secret, run end-to-end verification commands, and make the change folder ready for implementation handoff.

### Changes Required:

#### 1. Environment Defaults

**File**: `.env.example`

**Intent**: Document the auth configuration needed for local development and deployment.

**Contract**: Add the backend auth signing secret variable and any other auth-related env keys introduced by Phase 1. Keep secrets clearly marked as local examples rather than production-safe values.

#### 2. Compose Auth Environment

**File**: `docker-compose.yml`

**Intent**: Ensure the containerized backend starts with the auth variables required by the new login gate.

**Contract**: Add the auth signing secret to the backend service environment. Do not add public frontend secrets for the bearer token model.

#### 3. Plan and Verification Handoff

**File**: `context/changes/minimal-login-gate/plan.md`

**Intent**: Keep implementation verification explicit and tied to the phase progress contract.

**Contract**: Preserve one canonical `## Progress` section at the bottom of the plan. `/10x-implement` will mark entries done and append commit SHAs; do not add sidecar state files.

### Success Criteria:

#### Automated Verification:

- Backend build passes: `cargo build --manifest-path backend/Cargo.toml`
- Backend tests pass: `cargo test --manifest-path backend/Cargo.toml`
- Frontend build passes: `npm run build` in `frontend/`.
- Frontend lint passes: `npm run lint` in `frontend/`.

#### Manual Verification:

- Full local stack still starts with `docker compose up --build`.
- Browser flow verifies login, forced password change, telemetry access, logout, and token-expiry/invalid-token recovery.
- Public `/health` and unauthenticated `GET /api/telemetry` behavior are verified with curl or browser dev tools.

**Implementation Note**: After completing this phase and all automated verification passes, pause here for final manual confirmation that the protected-access foundation is ready to unlock `S-01`.

---

## Testing Strategy

### Unit Tests:

- Test Argon2 password verification accepts the seeded first-run password and rejects wrong passwords.
- Test login response distinguishes forced-change accounts from normal sessions.
- Test password change updates the stored hash, clears `must_change_password`, and rejects reuse of the limited token for dashboard telemetry.
- Test bearer token validation rejects missing, malformed, expired, wrong-secret, and wrong-session-kind tokens.

### Integration Tests:

- Exercise backend routes with an initialized test database or isolated test state: login, change password, session check, protected telemetry read, and unauthenticated telemetry rejection.
- Verify auth-unavailable behavior when `AppState.db` is absent.
- Verify public `/health` remains reachable without auth.

### Manual Testing Steps:

1. Start Postgres/backend/frontend locally.
2. Open the frontend and confirm the login form appears before any telemetry.
3. Log in with `admin` / `admin` and confirm telemetry is still hidden behind the password-change screen.
4. Change the password and confirm the dashboard loads telemetry.
5. Log out and confirm the stored token is cleared.
6. Manually corrupt or remove the stored token and confirm the app returns to login on the next protected fetch.
7. Curl `/health` without auth and confirm it succeeds.
8. Curl `/api/telemetry` without auth and confirm it returns `401 Unauthorized`.

## Performance Considerations

Auth checks add a small per-request token validation cost to telemetry reads. Password hashing only runs during login and password change, so Argon2 cost should be reasonable for a small after-hours MVP and should not affect telemetry polling.

## Migration Notes

The auth table is created through the existing inline startup migration pattern. Existing telemetry rows are unaffected. If Postgres already contains at least one fleet-manager user, startup must not recreate or reset the admin password.

If Postgres is unavailable, auth is unavailable by design. This is a deliberate divergence from telemetry's in-memory fallback because forced password-change state must stay durable.

## References

- Change identity: `context/changes/minimal-login-gate/change.md`
- Roadmap item: `context/foundation/roadmap.md` (`F-01: Minimal login gate`)
- Product access control: `context/foundation/prd.md`
- Original shaping decisions: `context/foundation/shape-notes.md`
- Backend routes and persistence: [backend/src/main.rs](/home/mmacura/Documents/Git/rc-dashboard/backend/src/main.rs:56)
- Frontend telemetry polling: [frontend/src/App.tsx](/home/mmacura/Documents/Git/rc-dashboard/frontend/src/App.tsx:24)
- Environment defaults: [.env.example](/home/mmacura/Documents/Git/rc-dashboard/.env.example:1)
- Compose backend environment: [docker-compose.yml](/home/mmacura/Documents/Git/rc-dashboard/docker-compose.yml:5)

## Progress

> Convention: `- [ ]` pending, `- [x]` done. Append ` — <commit sha>` when a step lands. Do not rename step titles. See `references/progress-format.md`.

### Phase 1: Backend Auth Foundation

#### Automated

- [x] 1.1 Backend compiles: `cargo build --manifest-path backend/Cargo.toml` — 005d2c2
- [x] 1.2 Backend auth tests pass: `cargo test --manifest-path backend/Cargo.toml` — 005d2c2
- [x] 1.3 `GET /api/telemetry` rejects requests without a normal bearer token in backend tests. — 005d2c2
- [x] 1.4 First-run `admin:admin` login returns password-change-required state in backend tests. — 005d2c2
- [x] 1.5 Password change clears the forced-change flag and returns a normal bearer token in backend tests. — 005d2c2
- [x] 1.6 Auth handlers return an unavailable response when Postgres/auth storage is unavailable in backend tests or documented smoke coverage. — 005d2c2

#### Manual

- [x] 1.7 With Postgres available, `admin:admin` can log in only far enough to change the password. — 005d2c2
- [x] 1.8 After password change, a normal token can access `GET /api/telemetry`. — 005d2c2
- [x] 1.9 Missing or invalid tokens cannot access `GET /api/telemetry`. — 005d2c2
- [x] 1.10 `/health` remains publicly reachable. — 005d2c2
- [x] 1.11 Simulator or MQTT telemetry ingestion still runs without fleet-manager credentials. — 005d2c2

### Phase 2: Frontend Login Gate

#### Automated

- [x] 2.1 Frontend build passes: `npm run build` in `frontend/`.
- [x] 2.2 Frontend lint passes: `npm run lint` in `frontend/`.

#### Manual

- [x] 2.3 Visiting the frontend without a token shows the login form instead of telemetry.
- [x] 2.4 Logging in with first-run `admin:admin` shows the password-change screen, not the dashboard.
- [x] 2.5 Changing the password transitions to the dashboard and telemetry loads.
- [x] 2.6 Logout clears the token and returns to login.
- [x] 2.7 Expired or invalid token behavior clears the session and returns to login.

### Phase 3: Config, Verification, and Documentation Alignment

#### Automated

- [ ] 3.1 Backend build passes: `cargo build --manifest-path backend/Cargo.toml`
- [ ] 3.2 Backend tests pass: `cargo test --manifest-path backend/Cargo.toml`
- [ ] 3.3 Frontend build passes: `npm run build` in `frontend/`.
- [ ] 3.4 Frontend lint passes: `npm run lint` in `frontend/`.

#### Manual

- [ ] 3.5 Full local stack still starts with `docker compose up --build`.
- [ ] 3.6 Browser flow verifies login, forced password change, telemetry access, logout, and token-expiry/invalid-token recovery.
- [ ] 3.7 Public `/health` and unauthenticated `GET /api/telemetry` behavior are verified with curl or browser dev tools.
