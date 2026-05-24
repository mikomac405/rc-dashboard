# Initial Railway Deployment Plan

## Summary

RC Smart Pit-Stop was deployed to Railway staging using the local CLI path. The deployment follows `context/foundation/infrastructure.md`: the Compose-shaped app is represented as separate Railway services, Postgres is a Railway database service, MQTT remains private, and only the frontend has a public domain.

## Railway Project

- Project: `rc-smart-pit-stop`
- Project ID: `[redacted]`
- Environment: `staging`
- Public frontend URL: `[redacted]`

## Services

- `Postgres`: Railway Postgres service, image `ghcr.io/railwayapp-templates/postgres-ssl:18`, volume `postgres-volume` mounted at `/var/lib/postgresql/data`.
- `mqtt`: deployed from `mqtt/Dockerfile`, private only, `PORT=1883`.
- `backend`: deployed from `backend/`, private only, listens on `[::]:3000`, uses `DATABASE_URL` from `Postgres` and `MQTT_HOST=mqtt.railway.internal`.
- `simulator`: deployed from `simulator/`, private only, publishes `rc/telemetry` to `mqtt.railway.internal:1883`.
- `frontend`: deployed from `frontend/`, public domain generated, proxies `/health` and `/api/*` to `backend.railway.internal:3000`.

## Commands Used

```bash
railway init -n rc-smart-pit-stop
railway environment new staging
railway environment link staging
railway add --database postgres
railway add --service backend
railway add --service frontend
railway add --service simulator
railway add --service mqtt
railway variable set 'API_HOST=[::]' API_PORT=3000 PORT=3000 'DATABASE_URL=${{Postgres.DATABASE_URL}}' MQTT_HOST=mqtt.railway.internal MQTT_PORT=1883 MQTT_TOPIC=rc/telemetry --service backend --environment staging --skip-deploys
railway variable set MQTT_HOST=mqtt.railway.internal MQTT_PORT=1883 MQTT_TOPIC=rc/telemetry CAR_ID=rc-07 --service simulator --environment staging --skip-deploys
railway variable set PORT=80 --service frontend --environment staging --skip-deploys
railway variable set PORT=1883 --service mqtt --environment staging --skip-deploys
railway up --service mqtt --environment staging --path-as-root ./mqtt
railway up --service backend --environment staging --path-as-root ./backend
railway up --service simulator --environment staging --path-as-root ./simulator
railway up --service frontend --environment staging --path-as-root ./frontend
railway domain --service frontend --environment staging --port 80
```

## Verification

- `curl -fsS [redacted]/health` returned `{"status":"ok",...}`.
- `curl -fsS [redacted]/api/telemetry` returned live telemetry rows for `rc-07`.
- `railway service list --json` showed `SUCCESS` for `Postgres`, `mqtt`, `backend`, `simulator`, and `frontend`.
- Backend logs showed `RC Smart Pit-Stop API listening on http://[::]:3000`.
- Simulator logs showed successful telemetry publishes.

## Notes

- The frontend initially crashed while backend was still building because Nginx attempted to resolve `backend.railway.internal` before the backend private DNS record was available. Railway restarted it automatically; once backend was running, frontend became healthy.
- No public domains were generated for `backend`, `mqtt`, `simulator`, or `Postgres`.
- Production was not created or deployed.
