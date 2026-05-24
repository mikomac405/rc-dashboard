use std::{
    collections::VecDeque,
    env,
    net::SocketAddr,
    sync::{Arc, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use rumqttc::{AsyncClient, Event, MqttOptions, Packet, QoS};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};
use tokio::time::sleep;
use tower_http::cors::CorsLayer;

const MAX_READINGS: usize = 120;

#[derive(Clone)]
struct AppState {
    readings: Arc<RwLock<VecDeque<TelemetryReading>>>,
    db: Option<PgPool>,
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

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    timestamp_ms: u128,
}

#[tokio::main]
async fn main() {
    let db = connect_database().await;
    let state = AppState {
        readings: Arc::new(RwLock::new(VecDeque::new())),
        db,
    };
    spawn_mqtt_consumer(state.clone());

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/telemetry", get(list_telemetry).post(record_telemetry))
        .route("/api/simulator/ingest", post(record_telemetry))
        .layer(CorsLayer::permissive())
        .with_state(state);

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

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        timestamp_ms: now_ms(),
    })
}

async fn list_telemetry(State(state): State<AppState>) -> Json<Vec<TelemetryReading>> {
    if let Some(db) = &state.db {
        match load_recent_readings(db).await {
            Ok(readings) => return Json(readings),
            Err(error) => eprintln!("database read failed, falling back to memory: {error}"),
        }
    }

    let readings = state.readings.read().expect("read telemetry state");
    Json(readings.iter().cloned().collect())
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

async fn migrate_database(pool: &PgPool) -> Result<(), sqlx::Error> {
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
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS telemetry_readings_timestamp_idx
        ON telemetry_readings (timestamp_ms DESC)
        "#,
    )
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
