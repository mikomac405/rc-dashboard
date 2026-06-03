use std::{
    collections::{HashSet, VecDeque},
    env,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand_core::OsRng;
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tokio::time::sleep;
use tower_http::cors::CorsLayer;

const MAX_READINGS: usize = 120;
const SESSION_TTL_SECONDS: u64 = 8 * 60 * 60;
const LOCAL_AUTH_SECRET: &str = "local-dev-change-me-minimal-login-gate";
const FIRST_RUN_USERNAME: &str = "admin";
const FIRST_RUN_PASSWORD: &str = "admin";
const STALE_HEARTBEAT_MS: u128 = 45_000;
const LOW_BATTERY_PERCENT: f32 = 20.0;
const OVERHEATED_MOTOR_TEMP_C: f32 = 85.0;
const SPEED_MISMATCH_KPH: f32 = 5.0;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone)]
struct AppState {
    readings: Arc<RwLock<VecDeque<TelemetryReading>>>,
    db: Option<PgPool>,
    auth: AuthConfig,
}

#[derive(Clone)]
struct AuthConfig {
    signing_secret: Arc<String>,
}

impl AuthConfig {
    fn from_env() -> Self {
        Self {
            signing_secret: Arc::new(
                env::var("AUTH_SIGNING_SECRET").unwrap_or_else(|_| LOCAL_AUTH_SECRET.to_string()),
            ),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct TelemetryReading {
    vehicle_id: String,
    health_state: HealthState,
    health_reason: HealthReason,
    battery_percent: f32,
    heartbeat_age_ms: u128,
    motor_temp_c: f32,
    commanded_speed_kph: f32,
    actual_speed_kph: f32,
    jam_detected: bool,
    mission_area: String,
    manual_pickup_required: bool,
    timestamp_ms: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum HealthState {
    Healthy,
    Unhealthy,
    Dead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum HealthReason {
    None,
    LowBattery,
    OverheatedMotor,
    SpeedMismatch,
    Jammed,
    LostConnection,
}

#[derive(Debug)]
struct FleetManagerUser {
    username: String,
    password_hash: String,
    must_change_password: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SessionKind {
    FleetManager,
    PasswordChange,
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionClaims {
    username: String,
    kind: SessionKind,
    exp: u64,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    new_password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    must_change_password: bool,
    expires_at_ms: u128,
}

#[derive(Serialize)]
struct SessionResponse {
    username: String,
    expires_at_ms: u128,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    timestamp_ms: u128,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: &'static str,
}

#[derive(Debug)]
enum AuthError {
    AuthUnavailable,
    BadRequest,
    Unauthorized,
    Storage,
    Token,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, error) = match self {
            AuthError::AuthUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "auth_unavailable"),
            AuthError::BadRequest => (StatusCode::BAD_REQUEST, "bad_request"),
            AuthError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            AuthError::Storage => (StatusCode::INTERNAL_SERVER_ERROR, "storage_error"),
            AuthError::Token => (StatusCode::INTERNAL_SERVER_ERROR, "token_error"),
        };

        (status, Json(ErrorResponse { error })).into_response()
    }
}

#[tokio::main]
async fn main() {
    let db = connect_database().await;
    let state = AppState {
        readings: Arc::new(RwLock::new(VecDeque::new())),
        db,
        auth: AuthConfig::from_env(),
    };
    spawn_mqtt_consumer(state.clone());

    let app = build_app(state);

    let host = env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("API_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(3000);
    let addr: SocketAddr = format!("{host}:{port}")
        .parse()
        .expect("parse API bind address");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("bind API listener");

    println!("RC Smart Pit-Stop API listening on http://{addr}");
    axum::serve(listener, app).await.expect("serve API");
}

fn build_app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/auth/login", post(login))
        .route("/api/auth/change-password", post(change_password))
        .route("/api/auth/me", get(auth_me))
        .route("/api/telemetry", get(list_telemetry))
        // Explicit HTTP ingest exists for manual and automated contract tests.
        // The normal telemetry path is simulator -> MQTT -> backend consumer.
        .route("/api/simulator/ingest", post(record_telemetry))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        timestamp_ms: now_ms(),
    })
}

async fn login(
    State(state): State<AppState>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    let db = state.db.as_ref().ok_or(AuthError::AuthUnavailable)?;
    let user = load_fleet_manager_user(db, &request.username)
        .await
        .map_err(|error| {
            eprintln!("fleet manager lookup failed: {error}");
            AuthError::Storage
        })?
        .ok_or(AuthError::Unauthorized)?;

    let response = authenticate_user(&user, &request.password, &state.auth)?;
    Ok(Json(response))
}

async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChangePasswordRequest>,
) -> Result<Json<LoginResponse>, AuthError> {
    let db = state.db.as_ref().ok_or(AuthError::AuthUnavailable)?;
    let claims = require_session(&headers, &state.auth, SessionKind::PasswordChange)?;

    if request.new_password.trim().is_empty() {
        return Err(AuthError::BadRequest);
    }

    let password_hash = hash_password(&request.new_password).map_err(|error| {
        eprintln!("password hashing failed: {error}");
        AuthError::Storage
    })?;

    update_fleet_manager_password(db, &claims.username, &password_hash)
        .await
        .map_err(|error| {
            eprintln!("password update failed: {error}");
            AuthError::Storage
        })?;

    let token = issue_token(&state.auth, &claims.username, SessionKind::FleetManager)?;
    Ok(Json(LoginResponse {
        token,
        must_change_password: false,
        expires_at_ms: (now_seconds() + SESSION_TTL_SECONDS) as u128 * 1000,
    }))
}

async fn auth_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<SessionResponse>, AuthError> {
    state.db.as_ref().ok_or(AuthError::AuthUnavailable)?;
    let claims = require_session(&headers, &state.auth, SessionKind::FleetManager)?;
    Ok(Json(SessionResponse {
        username: claims.username,
        expires_at_ms: claims.exp as u128 * 1000,
    }))
}

async fn list_telemetry(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<TelemetryReading>>, AuthError> {
    require_session(&headers, &state.auth, SessionKind::FleetManager)?;

    if let Some(db) = &state.db {
        match load_recent_readings(db).await {
            Ok(readings) => return Ok(Json(apply_freshness_states(readings, now_ms()))),
            Err(error) => eprintln!("database read failed, falling back to memory: {error}"),
        }
    } else {
        return Err(AuthError::AuthUnavailable);
    }

    let readings = state.readings.read().expect("read telemetry state");
    Ok(Json(apply_freshness_states(
        readings.iter().cloned().collect(),
        now_ms(),
    )))
}

async fn record_telemetry(
    State(state): State<AppState>,
    Json(mut reading): Json<TelemetryReading>,
) -> StatusCode {
    normalize_reading(&mut reading);
    persist_reading(&state, reading).await;
    StatusCode::ACCEPTED
}

async fn connect_database() -> Option<PgPool> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://rc_dashboard:rc_dashboard@localhost:5432/rc_dashboard".to_string()
    });

    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(2))
        .connect(&database_url)
        .await
    {
        Ok(pool) => pool,
        Err(error) => {
            eprintln!("database unavailable, continuing with in-memory telemetry: {error}");
            return None;
        }
    };

    if let Err(error) = migrate_database(&pool).await {
        eprintln!("database migration failed, continuing with in-memory telemetry: {error}");
        return None;
    }

    Some(pool)
}

async fn migrate_database(pool: &PgPool) -> Result<(), String> {
    rebuild_old_telemetry_schema(pool).await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS telemetry_readings (
            id BIGSERIAL PRIMARY KEY,
            vehicle_id TEXT NOT NULL,
            health_state TEXT NOT NULL,
            health_reason TEXT NOT NULL,
            battery_percent REAL NOT NULL,
            heartbeat_age_ms BIGINT NOT NULL,
            motor_temp_c REAL NOT NULL,
            commanded_speed_kph REAL NOT NULL,
            actual_speed_kph REAL NOT NULL,
            jam_detected BOOLEAN NOT NULL,
            mission_area TEXT NOT NULL,
            manual_pickup_required BOOLEAN NOT NULL,
            timestamp_ms BIGINT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS telemetry_readings_timestamp_idx
        ON telemetry_readings (timestamp_ms DESC)
        "#,
    )
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS fleet_manager_users (
            id BIGSERIAL PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            must_change_password BOOLEAN NOT NULL DEFAULT true,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    seed_first_run_admin(pool).await?;

    Ok(())
}

async fn rebuild_old_telemetry_schema(pool: &PgPool) -> Result<(), String> {
    let has_telemetry_table = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = 'telemetry_readings'
        ) AS table_exists
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?
    .get::<bool, _>("table_exists");

    if !has_telemetry_table {
        return Ok(());
    }

    let has_vehicle_id = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = 'telemetry_readings'
              AND column_name = 'vehicle_id'
        ) AS column_exists
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?
    .get::<bool, _>("column_exists");

    if !has_vehicle_id {
        sqlx::query("DROP TABLE telemetry_readings")
            .execute(pool)
            .await
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

async fn seed_first_run_admin(pool: &PgPool) -> Result<(), String> {
    let user_count = sqlx::query("SELECT COUNT(*) AS user_count FROM fleet_manager_users")
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?
        .get::<i64, _>("user_count");

    if user_count > 0 {
        return Ok(());
    }

    let password_hash = hash_password(FIRST_RUN_PASSWORD).map_err(|error| error.to_string())?;
    sqlx::query(
        r#"
        INSERT INTO fleet_manager_users (username, password_hash, must_change_password)
        VALUES ($1, $2, true)
        "#,
    )
    .bind(FIRST_RUN_USERNAME)
    .bind(password_hash)
    .execute(pool)
    .await
    .map_err(|error| error.to_string())?;

    Ok(())
}

async fn load_fleet_manager_user(
    pool: &PgPool,
    username: &str,
) -> Result<Option<FleetManagerUser>, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT username, password_hash, must_change_password
        FROM fleet_manager_users
        WHERE username = $1
        "#,
    )
    .bind(username)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|row| FleetManagerUser {
        username: row.get("username"),
        password_hash: row.get("password_hash"),
        must_change_password: row.get("must_change_password"),
    }))
}

async fn update_fleet_manager_password(
    pool: &PgPool,
    username: &str,
    password_hash: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE fleet_manager_users
        SET password_hash = $1, must_change_password = false, updated_at = now()
        WHERE username = $2
        "#,
    )
    .bind(password_hash)
    .bind(username)
    .execute(pool)
    .await?;

    Ok(())
}

async fn load_recent_readings(pool: &PgPool) -> Result<Vec<TelemetryReading>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT vehicle_id, health_state, health_reason, battery_percent,
               heartbeat_age_ms, motor_temp_c, commanded_speed_kph,
               actual_speed_kph, jam_detected, mission_area,
               manual_pickup_required, timestamp_ms
        FROM telemetry_readings
        ORDER BY timestamp_ms DESC
        LIMIT $1
        "#,
    )
    .bind(MAX_READINGS as i64)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| TelemetryReading {
            vehicle_id: row.get("vehicle_id"),
            health_state: parse_health_state(row.get("health_state")),
            health_reason: parse_health_reason(row.get("health_reason")),
            battery_percent: row.get("battery_percent"),
            heartbeat_age_ms: row.get::<i64, _>("heartbeat_age_ms") as u128,
            motor_temp_c: row.get("motor_temp_c"),
            commanded_speed_kph: row.get("commanded_speed_kph"),
            actual_speed_kph: row.get("actual_speed_kph"),
            jam_detected: row.get("jam_detected"),
            mission_area: row.get("mission_area"),
            manual_pickup_required: row.get("manual_pickup_required"),
            timestamp_ms: row.get::<i64, _>("timestamp_ms") as u128,
        })
        .collect())
}

async fn insert_reading(pool: &PgPool, reading: &TelemetryReading) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO telemetry_readings (
            vehicle_id, health_state, health_reason, battery_percent,
            heartbeat_age_ms, motor_temp_c, commanded_speed_kph,
            actual_speed_kph, jam_detected, mission_area, manual_pickup_required,
            timestamp_ms
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(&reading.vehicle_id)
    .bind(health_state_name(reading.health_state))
    .bind(health_reason_name(reading.health_reason))
    .bind(reading.battery_percent)
    .bind(reading.heartbeat_age_ms as i64)
    .bind(reading.motor_temp_c)
    .bind(reading.commanded_speed_kph)
    .bind(reading.actual_speed_kph)
    .bind(reading.jam_detected)
    .bind(&reading.mission_area)
    .bind(reading.manual_pickup_required)
    .bind(reading.timestamp_ms as i64)
    .execute(pool)
    .await?;

    Ok(())
}

fn normalize_reading(reading: &mut TelemetryReading) {
    let (state, reason, manual_pickup_required) = evaluate_health(reading);
    reading.health_state = state;
    reading.health_reason = reason;
    reading.manual_pickup_required = manual_pickup_required;
}

fn evaluate_health(reading: &TelemetryReading) -> (HealthState, HealthReason, bool) {
    if reading.heartbeat_age_ms >= STALE_HEARTBEAT_MS {
        return (HealthState::Dead, HealthReason::LostConnection, true);
    }

    if reading.jam_detected {
        return (HealthState::Unhealthy, HealthReason::Jammed, true);
    }

    if reading.battery_percent <= LOW_BATTERY_PERCENT {
        return (HealthState::Unhealthy, HealthReason::LowBattery, false);
    }

    if reading.motor_temp_c >= OVERHEATED_MOTOR_TEMP_C {
        return (HealthState::Unhealthy, HealthReason::OverheatedMotor, false);
    }

    if speed_mismatch(reading) >= SPEED_MISMATCH_KPH {
        return (HealthState::Unhealthy, HealthReason::SpeedMismatch, false);
    }

    (HealthState::Healthy, HealthReason::None, false)
}

fn apply_freshness_states(
    mut readings: Vec<TelemetryReading>,
    now_ms: u128,
) -> Vec<TelemetryReading> {
    let mut seen_vehicles = HashSet::new();
    let mut synthesized_dead = Vec::new();

    for reading in &readings {
        if !seen_vehicles.insert(reading.vehicle_id.clone()) {
            continue;
        }

        if reading.health_state == HealthState::Dead {
            continue;
        }

        if now_ms.saturating_sub(reading.timestamp_ms) >= STALE_HEARTBEAT_MS {
            let mut dead_reading = reading.clone();
            dead_reading.health_state = HealthState::Dead;
            dead_reading.health_reason = HealthReason::LostConnection;
            dead_reading.manual_pickup_required = true;
            dead_reading.heartbeat_age_ms = now_ms.saturating_sub(reading.timestamp_ms);
            dead_reading.timestamp_ms = now_ms;
            synthesized_dead.push(dead_reading);
        }
    }

    if synthesized_dead.is_empty() {
        return readings;
    }

    synthesized_dead.append(&mut readings);
    synthesized_dead.truncate(MAX_READINGS);
    synthesized_dead
}

fn speed_mismatch(reading: &TelemetryReading) -> f32 {
    (reading.commanded_speed_kph - reading.actual_speed_kph).abs()
}

fn health_state_name(state: HealthState) -> &'static str {
    match state {
        HealthState::Healthy => "healthy",
        HealthState::Unhealthy => "unhealthy",
        HealthState::Dead => "dead",
    }
}

fn parse_health_state(value: String) -> HealthState {
    match value.as_str() {
        "unhealthy" => HealthState::Unhealthy,
        "dead" => HealthState::Dead,
        _ => HealthState::Healthy,
    }
}

fn health_reason_name(reason: HealthReason) -> &'static str {
    match reason {
        HealthReason::None => "none",
        HealthReason::LowBattery => "low_battery",
        HealthReason::OverheatedMotor => "overheated_motor",
        HealthReason::SpeedMismatch => "speed_mismatch",
        HealthReason::Jammed => "jammed",
        HealthReason::LostConnection => "lost_connection",
    }
}

fn parse_health_reason(value: String) -> HealthReason {
    match value.as_str() {
        "low_battery" => HealthReason::LowBattery,
        "overheated_motor" => HealthReason::OverheatedMotor,
        "speed_mismatch" => HealthReason::SpeedMismatch,
        "jammed" => HealthReason::Jammed,
        "lost_connection" => HealthReason::LostConnection,
        _ => HealthReason::None,
    }
}

fn authenticate_user(
    user: &FleetManagerUser,
    password: &str,
    auth: &AuthConfig,
) -> Result<LoginResponse, AuthError> {
    if !verify_password(password, &user.password_hash) {
        return Err(AuthError::Unauthorized);
    }

    let kind = if user.must_change_password {
        SessionKind::PasswordChange
    } else {
        SessionKind::FleetManager
    };
    let token = issue_token(auth, &user.username, kind)?;

    Ok(LoginResponse {
        token,
        must_change_password: user.must_change_password,
        expires_at_ms: (now_seconds() + SESSION_TTL_SECONDS) as u128 * 1000,
    })
}

fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)?
        .to_string())
}

fn verify_password(password: &str, password_hash: &str) -> bool {
    let Ok(parsed_hash) = PasswordHash::new(password_hash) else {
        return false;
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

fn issue_token(auth: &AuthConfig, username: &str, kind: SessionKind) -> Result<String, AuthError> {
    let claims = SessionClaims {
        username: username.to_string(),
        kind,
        exp: now_seconds() + SESSION_TTL_SECONDS,
    };
    encode_token(auth, &claims)
}

fn encode_token(auth: &AuthConfig, claims: &SessionClaims) -> Result<String, AuthError> {
    let payload = serde_json::to_vec(claims).map_err(|_| AuthError::Token)?;
    let encoded_payload = URL_SAFE_NO_PAD.encode(payload);
    let signature = sign_token_payload(auth, &encoded_payload)?;

    Ok(format!("{encoded_payload}.{signature}"))
}

fn decode_token(auth: &AuthConfig, token: &str) -> Result<SessionClaims, AuthError> {
    let (encoded_payload, encoded_signature) =
        token.split_once('.').ok_or(AuthError::Unauthorized)?;
    let expected_signature = sign_token_payload(auth, encoded_payload)?;

    if expected_signature != encoded_signature {
        return Err(AuthError::Unauthorized);
    }

    let payload = URL_SAFE_NO_PAD
        .decode(encoded_payload)
        .map_err(|_| AuthError::Unauthorized)?;
    let claims: SessionClaims =
        serde_json::from_slice(&payload).map_err(|_| AuthError::Unauthorized)?;

    if claims.exp <= now_seconds() {
        return Err(AuthError::Unauthorized);
    }

    Ok(claims)
}

fn sign_token_payload(auth: &AuthConfig, encoded_payload: &str) -> Result<String, AuthError> {
    let mut mac =
        HmacSha256::new_from_slice(auth.signing_secret.as_bytes()).map_err(|_| AuthError::Token)?;
    mac.update(encoded_payload.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
}

fn require_session(
    headers: &HeaderMap,
    auth: &AuthConfig,
    required_kind: SessionKind,
) -> Result<SessionClaims, AuthError> {
    let token = bearer_token(headers).ok_or(AuthError::Unauthorized)?;
    let claims = decode_token(auth, token)?;

    if claims.kind != required_kind {
        return Err(AuthError::Unauthorized);
    }

    Ok(claims)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
}

fn spawn_mqtt_consumer(state: AppState) {
    tokio::spawn(async move {
        let host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = env::var("MQTT_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(1883);
        let topic = env::var("MQTT_TOPIC").unwrap_or_else(|_| "rc/telemetry".to_string());

        loop {
            let mut mqtt_options = MqttOptions::new("rc-smart-pit-stop-api", &host, port);
            mqtt_options.set_keep_alive(Duration::from_secs(20));

            let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

            if let Err(error) = client.subscribe(&topic, QoS::AtLeastOnce).await {
                eprintln!("MQTT subscribe failed: {error}");
                sleep(Duration::from_secs(20)).await;
                continue;
            }

            loop {
                match event_loop.poll().await {
                    Ok(Event::Incoming(Packet::Publish(message))) => {
                        match serde_json::from_slice::<TelemetryReading>(&message.payload) {
                            Ok(reading) => persist_reading(&state, reading).await,
                            Err(error) => eprintln!("invalid telemetry payload: {error}"),
                        }
                    }
                    Ok(_) => {}
                    Err(error) => {
                        eprintln!("MQTT consumer disconnected: {error}");
                        sleep(Duration::from_secs(60)).await;
                        break;
                    }
                }
            }
        }
    });
}

async fn persist_reading(state: &AppState, reading: TelemetryReading) {
    if let Some(db) = &state.db {
        if let Err(error) = insert_reading(db, &reading).await {
            eprintln!("database insert failed, keeping reading in memory only: {error}");
        }
    }

    let mut readings = state.readings.write().expect("write telemetry state");
    readings.push_front(reading);
    readings.truncate(MAX_READINGS);
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis()
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            readings: Arc::new(RwLock::new(VecDeque::new())),
            db: None,
            auth: AuthConfig {
                signing_secret: Arc::new("test-secret".to_string()),
            },
        }
    }

    fn sample_health_reading() -> TelemetryReading {
        TelemetryReading {
            vehicle_id: "rc-07".to_string(),
            health_state: HealthState::Healthy,
            health_reason: HealthReason::None,
            battery_percent: 82.0,
            heartbeat_age_ms: 1_000,
            motor_temp_c: 52.0,
            commanded_speed_kph: 18.0,
            actual_speed_kph: 17.4,
            jam_detected: false,
            mission_area: "sector-a".to_string(),
            manual_pickup_required: false,
            timestamp_ms: 10_000,
        }
    }

    #[test]
    fn password_hash_accepts_correct_password_and_rejects_wrong_password() {
        let password_hash = hash_password(FIRST_RUN_PASSWORD).expect("hash first-run password");

        assert!(verify_password(FIRST_RUN_PASSWORD, &password_hash));
        assert!(!verify_password("not-admin", &password_hash));
    }

    #[test]
    fn first_run_admin_login_requires_password_change() {
        let auth = AuthConfig {
            signing_secret: Arc::new("test-secret".to_string()),
        };
        let user = FleetManagerUser {
            username: FIRST_RUN_USERNAME.to_string(),
            password_hash: hash_password(FIRST_RUN_PASSWORD).expect("hash first-run password"),
            must_change_password: true,
        };

        let response =
            authenticate_user(&user, FIRST_RUN_PASSWORD, &auth).expect("authenticate first run");
        let claims = decode_token(&auth, &response.token).expect("decode issued token");

        assert!(response.must_change_password);
        assert_eq!(claims.kind, SessionKind::PasswordChange);
        assert_eq!(claims.username, FIRST_RUN_USERNAME);
    }

    #[test]
    fn changed_password_login_returns_normal_session() {
        let auth = AuthConfig {
            signing_secret: Arc::new("test-secret".to_string()),
        };
        let user = FleetManagerUser {
            username: FIRST_RUN_USERNAME.to_string(),
            password_hash: hash_password("new-password").expect("hash changed password"),
            must_change_password: false,
        };

        let response =
            authenticate_user(&user, "new-password", &auth).expect("authenticate changed password");
        let claims = decode_token(&auth, &response.token).expect("decode issued token");

        assert!(!response.must_change_password);
        assert_eq!(claims.kind, SessionKind::FleetManager);
    }

    #[test]
    fn password_change_token_cannot_access_normal_session() {
        let auth = AuthConfig {
            signing_secret: Arc::new("test-secret".to_string()),
        };
        let token =
            issue_token(&auth, FIRST_RUN_USERNAME, SessionKind::PasswordChange).expect("token");
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {token}").parse().expect("valid header"),
        );

        assert!(require_session(&headers, &auth, SessionKind::FleetManager).is_err());
        assert!(require_session(&headers, &auth, SessionKind::PasswordChange).is_ok());
    }

    #[test]
    fn token_validation_rejects_wrong_secret_and_expired_tokens() {
        let auth = AuthConfig {
            signing_secret: Arc::new("test-secret".to_string()),
        };
        let other_auth = AuthConfig {
            signing_secret: Arc::new("other-secret".to_string()),
        };
        let token = issue_token(&auth, FIRST_RUN_USERNAME, SessionKind::FleetManager)
            .expect("issue normal token");
        let expired = encode_token(
            &auth,
            &SessionClaims {
                username: FIRST_RUN_USERNAME.to_string(),
                kind: SessionKind::FleetManager,
                exp: now_seconds() - 1,
            },
        )
        .expect("issue expired token");

        assert!(decode_token(&auth, &token).is_ok());
        assert!(decode_token(&other_auth, &token).is_err());
        assert!(decode_token(&auth, &expired).is_err());
        assert!(decode_token(&auth, "not-a-token").is_err());
    }

    #[test]
    fn stale_heartbeat_produces_dead_lost_connection_manual_pickup() {
        let mut reading = sample_health_reading();
        reading.heartbeat_age_ms = STALE_HEARTBEAT_MS;

        normalize_reading(&mut reading);

        assert_eq!(reading.health_state, HealthState::Dead);
        assert_eq!(reading.health_reason, HealthReason::LostConnection);
        assert!(reading.manual_pickup_required);
    }

    #[test]
    fn missing_fresh_message_surfaces_known_vehicle_as_dead() {
        let reading = sample_health_reading();
        let readings = apply_freshness_states(vec![reading], 10_000 + STALE_HEARTBEAT_MS);

        assert_eq!(readings[0].vehicle_id, "rc-07");
        assert_eq!(readings[0].health_state, HealthState::Dead);
        assert_eq!(readings[0].health_reason, HealthReason::LostConnection);
        assert!(readings[0].manual_pickup_required);
    }

    #[test]
    fn jam_detection_produces_unhealthy_jammed_manual_pickup() {
        let mut reading = sample_health_reading();
        reading.jam_detected = true;
        reading.actual_speed_kph = 0.0;

        normalize_reading(&mut reading);

        assert_eq!(reading.health_state, HealthState::Unhealthy);
        assert_eq!(reading.health_reason, HealthReason::Jammed);
        assert!(reading.manual_pickup_required);
    }

    #[test]
    fn healthy_reading_stays_healthy_without_manual_pickup() {
        let mut reading = sample_health_reading();

        normalize_reading(&mut reading);

        assert_eq!(reading.health_state, HealthState::Healthy);
        assert_eq!(reading.health_reason, HealthReason::None);
        assert!(!reading.manual_pickup_required);
    }

    #[tokio::test]
    async fn telemetry_rejects_missing_bearer_token() {
        let response = build_app(test_state())
            .oneshot(
                Request::builder()
                    .uri("/api/telemetry")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn telemetry_post_is_not_an_ingest_route() {
        let response = build_app(test_state())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/telemetry")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn auth_routes_report_unavailable_without_database() {
        let response = build_app(test_state())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"username":"admin","password":"admin"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn health_remains_public() {
        let response = build_app(test_state())
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");

        assert_eq!(response.status(), StatusCode::OK);
    }
}
