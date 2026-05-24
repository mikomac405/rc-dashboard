use std::{env, time::Duration};

use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde::Serialize;
use tokio::time::sleep;

#[derive(Serialize)]
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

#[tokio::main]
async fn main() {
    let host = env::var("MQTT_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port = env::var("MQTT_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(1883);
    let car_id = env::var("CAR_ID").unwrap_or_else(|_| "rc-07".to_string());
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

    let mut lap = 1_u32;

    loop {
        let reading = sample_reading(&car_id, lap);
        let payload = serde_json::to_vec(&reading).expect("serialize telemetry");

        match client
            .publish(&topic, QoS::AtLeastOnce, false, payload)
            .await
        {
            Ok(()) => {
                println!(
                    "published car={} lap={} wear={:.1}% battery={:.1}C pit={}",
                    reading.car_id,
                    reading.lap,
                    reading.tire_wear,
                    reading.battery_temp_c,
                    reading.pit_recommended
                );
            }
            Err(error) => eprintln!("publish failed: {error}"),
        }

        lap += 1;
        sleep(Duration::from_secs(20)).await;
    }
}

fn sample_reading(car_id: &str, lap: u32) -> TelemetryReading {
    let lap_wave = (lap as f32 / 3.0).sin();
    let tire_wear = (8.0 + lap as f32 * 3.7 + lap_wave * 4.0).min(100.0);
    let battery_temp_c = 36.0 + lap as f32 * 0.8 + lap_wave * 3.0;
    let motor_temp_c = 52.0 + lap as f32 * 1.1 + lap_wave * 4.5;
    let speed_kph = 42.0 + lap_wave * 7.0 - (tire_wear / 20.0);

    TelemetryReading {
        car_id: car_id.to_string(),
        lap,
        tire_wear,
        battery_temp_c,
        motor_temp_c,
        speed_kph,
        pit_recommended: tire_wear >= 72.0 || battery_temp_c >= 58.0 || motor_temp_c >= 84.0,
        timestamp_ms: now_ms(),
    }
}

fn now_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_millis()
}
