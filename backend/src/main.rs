use std::{
    collections::VecDeque,
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
    car_id: String,
    lap: u32,
    tire_wear: f32,
    battery_temp_c: f32,
    motor_temp_c: f32,
    speed_kph: f32,
    pit_recommended: bool,
    timestamp_ms: u128,
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
        .route("/api/telemetry", get(list_telemetry).post(record_telemetry))
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
            Ok(readings) => return Ok(Json(readings)),
            Err(error) => eprintln!("database read failed, falling back to memory: {error}"),
        }
    } else {
        return Err(AuthError::AuthUnavailable);
    }

    let readings = state.readings.read().expect("read telemetry state");
    Ok(Json(readings.iter().cloned().collect()))
}

async fn record_telemetry(
    State(state): State<AppState>,
    Json(reading): Json<TelemetryReading>,
) -> StatusCode {
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
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS telemetry_readings (
            id BIGSERIAL PRIMARY KEY,
            car_id TEXT NOT NULL,
            lap INTEGER NOT NULL,
            tire_wear REAL NOT NULL,
            battery_temp_c REAL NOT NULL,
            motor_temp_c REAL NOT NULL,
            speed_kph REAL NOT NULL,
            pit_recommended BOOLEAN NOT NULL,
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
        SELECT car_id, lap, tire_wear, battery_temp_c, motor_temp_c, speed_kph,
               pit_recommended, timestamp_ms
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
            car_id: row.get("car_id"),
            lap: row.get::<i32, _>("lap") as u32,
            tire_wear: row.get("tire_wear"),
            battery_temp_c: row.get("battery_temp_c"),
            motor_temp_c: row.get("motor_temp_c"),
            speed_kph: row.get("speed_kph"),
            pit_recommended: row.get("pit_recommended"),
            timestamp_ms: row.get::<i64, _>("timestamp_ms") as u128,
        })
        .collect())
}

async fn insert_reading(pool: &PgPool, reading: &TelemetryReading) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO telemetry_readings (
            car_id, lap, tire_wear, battery_temp_c, motor_temp_c, speed_kph,
            pit_recommended, timestamp_ms
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
    )
    .bind(&reading.car_id)
    .bind(reading.lap as i32)
    .bind(reading.tire_wear)
    .bind(reading.battery_temp_c)
    .bind(reading.motor_temp_c)
    .bind(reading.speed_kph)
    .bind(reading.pit_recommended)
    .bind(reading.timestamp_ms as i64)
    .execute(pool)
    .await?;

    Ok(())
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
