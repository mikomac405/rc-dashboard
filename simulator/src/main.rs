use std::{env, time::Duration};

use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use tokio::time::sleep;

const STALE_HEARTBEAT_MS: u128 = 45_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum HealthState {
    Healthy,
    Unhealthy,
    Dead,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum HealthReason {
    None,
    LowBattery,
    SpeedMismatch,
    Jammed,
    LostConnection,
}

#[derive(Debug, Serialize)]
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

#[tokio::main]
async fn main() {
    let host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MQTT_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(1883);
    let vehicle_id = env::var("CAR_ID").unwrap_or_else(|_| "rc-07".to_string());
    let topic = env::var("MQTT_TOPIC").unwrap_or_else(|_| "rc/telemetry".to_string());

    let mut mqtt_options = MqttOptions::new("rc-smart-pit-stop-simulator", host, port);
    mqtt_options.set_keep_alive(Duration::from_secs(10));

    let (client, mut event_loop) = AsyncClient::new(mqtt_options, 10);

    tokio::spawn(async move {
        loop {
            if let Err(error) = event_loop.poll().await {
                eprintln!("MQTT event loop error: {error}");
                sleep(Duration::from_secs(1)).await;
            }
        }
    });

    let mut sample_index = 0_u32;

    loop {
        let reading = sample_reading(&vehicle_id, sample_index);
        let payload = serde_json::to_vec(&reading).expect("serialize telemetry");

        match client
            .publish(&topic, QoS::AtLeastOnce, false, payload)
            .await
        {
            Ok(()) => {
                println!(
                    "published vehicle={} area={} state={:?} reason={:?} battery={:.1}% motor={:.1}C jam={} pickup={}",
                    reading.vehicle_id,
                    reading.mission_area,
                    reading.health_state,
                    reading.health_reason,
                    reading.battery_percent,
                    reading.motor_temp_c,
                    reading.jam_detected,
                    reading.manual_pickup_required
                );
            }
            Err(error) => eprintln!("publish failed: {error}"),
        }

        sample_index = sample_index.wrapping_add(1);
        sleep(Duration::from_secs(20)).await;
    }
}

fn sample_reading(vehicle_id: &str, sample_index: u32) -> TelemetryReading {
    let mission_area = match sample_index % 4 {
        0 => "sector-alpha",
        1 => "sector-beta",
        2 => "sector-gamma",
        _ => "sector-delta",
    };

    match sample_index % 5 {
        0 => TelemetryReading {
            vehicle_id: vehicle_id.to_string(),
            health_state: HealthState::Healthy,
            health_reason: HealthReason::None,
            battery_percent: 82.0,
            heartbeat_age_ms: 1_000,
            motor_temp_c: 52.0,
            commanded_speed_kph: 12.0,
            actual_speed_kph: 11.7,
            jam_detected: false,
            mission_area: mission_area.to_string(),
            manual_pickup_required: false,
            timestamp_ms: now_ms(),
        },
        1 => TelemetryReading {
            vehicle_id: vehicle_id.to_string(),
            health_state: HealthState::Unhealthy,
            health_reason: HealthReason::LowBattery,
            battery_percent: 15.0,
            heartbeat_age_ms: 1_000,
            motor_temp_c: 56.0,
            commanded_speed_kph: 10.0,
            actual_speed_kph: 9.8,
            jam_detected: false,
            mission_area: mission_area.to_string(),
            manual_pickup_required: false,
            timestamp_ms: now_ms(),
        },
        2 => TelemetryReading {
            vehicle_id: vehicle_id.to_string(),
            health_state: HealthState::Unhealthy,
            health_reason: HealthReason::SpeedMismatch,
            battery_percent: 66.0,
            heartbeat_age_ms: 1_000,
            motor_temp_c: 58.0,
            commanded_speed_kph: 14.0,
            actual_speed_kph: 6.0,
            jam_detected: false,
            mission_area: mission_area.to_string(),
            manual_pickup_required: false,
            timestamp_ms: now_ms(),
        },
        3 => TelemetryReading {
            vehicle_id: vehicle_id.to_string(),
            health_state: HealthState::Unhealthy,
            health_reason: HealthReason::Jammed,
            battery_percent: 70.0,
            heartbeat_age_ms: 1_000,
            motor_temp_c: 60.0,
            commanded_speed_kph: 10.0,
            actual_speed_kph: 0.0,
            jam_detected: true,
            mission_area: mission_area.to_string(),
            manual_pickup_required: true,
            timestamp_ms: now_ms(),
        },
        _ => TelemetryReading {
            vehicle_id: vehicle_id.to_string(),
            health_state: HealthState::Dead,
            health_reason: HealthReason::LostConnection,
            battery_percent: 64.0,
            heartbeat_age_ms: STALE_HEARTBEAT_MS,
            motor_temp_c: 48.0,
            commanded_speed_kph: 0.0,
            actual_speed_kph: 0.0,
            jam_detected: false,
            mission_area: mission_area.to_string(),
            manual_pickup_required: true,
            timestamp_ms: now_ms(),
        },
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_cycle_includes_jammed_and_dead_scenarios() {
        let jammed = sample_reading("rc-test", 3);
        let dead = sample_reading("rc-test", 4);

        assert_eq!(jammed.health_state, HealthState::Unhealthy);
        assert_eq!(jammed.health_reason, HealthReason::Jammed);
        assert!(jammed.manual_pickup_required);

        assert_eq!(dead.health_state, HealthState::Dead);
        assert_eq!(dead.health_reason, HealthReason::LostConnection);
        assert!(dead.manual_pickup_required);
    }
}
