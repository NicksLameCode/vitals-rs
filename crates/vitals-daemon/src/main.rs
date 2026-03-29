mod dbus;

use std::sync::{Arc, Mutex};
use std::time::Instant;

use vitals_core::config::AppConfig;
use vitals_core::history::TimeSeriesStore;
use vitals_core::sensors::{SensorManager, SensorValue};

fn main() -> anyhow::Result<()> {
    env_logger::init();
    log::info!("Vitals daemon starting...");

    let config = Arc::new(AppConfig::load());
    let mut sensor_manager = SensorManager::new(&config);

    let sensor_data = Arc::new(Mutex::new(dbus::SensorData::new()));
    let time_series = Arc::new(Mutex::new(TimeSeriesStore::new(
        config.history.duration_seconds,
    )));

    // Load cached history
    let history_path = AppConfig::history_path();
    if let Ok(mut ts) = time_series.lock() {
        let _ = ts.load(&history_path);
    }

    // Set up D-Bus
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let sensor_data_clone = sensor_data.clone();
    let time_series_clone = time_series.clone();

    // Run D-Bus server in a separate thread
    let _dbus_handle = std::thread::spawn(move || {
        rt.block_on(async {
            let vitals = dbus::VitalsSensors {
                data: sensor_data_clone,
                time_series: time_series_clone,
            };

            let _connection = zbus::connection::Builder::session()
                .expect("Failed to create D-Bus connection builder")
                .name("com.corecoding.Vitals")
                .expect("Failed to set D-Bus name")
                .serve_at("/com/corecoding/Vitals", vitals)
                .expect("Failed to serve D-Bus interface")
                .build()
                .await
                .expect("Failed to build D-Bus connection");

            log::info!("D-Bus service registered at com.corecoding.Vitals");

            // Keep the connection alive
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        });
    });

    // Main polling loop
    let update_interval = std::time::Duration::from_secs(config.general.update_time as u64);
    let mut last_query = Instant::now();

    log::info!(
        "Starting sensor polling every {} seconds",
        config.general.update_time
    );

    loop {
        let now = Instant::now();
        let dwell = now.duration_since(last_query).as_secs_f64();
        last_query = now;

        let readings = sensor_manager.query_all(dwell);

        // Update shared sensor data
        if let Ok(mut data) = sensor_data.lock() {
            data.readings.clear();
            data.text_readings.clear();
            for reading in &readings {
                let cat_str = reading.category.to_string();
                let fmt_str = reading.format.as_str().to_string();
                match &reading.value {
                    SensorValue::Numeric(v) => {
                        data.readings.insert(
                            reading.key.clone(),
                            (reading.label.clone(), *v, cat_str, fmt_str),
                        );
                    }
                    SensorValue::Text(t) => {
                        data.text_readings.insert(
                            reading.key.clone(),
                            (reading.label.clone(), t.clone(), cat_str, fmt_str),
                        );
                    }
                    SensorValue::Disabled => {}
                }
            }
        }

        // Update time series
        if let Ok(mut ts) = time_series.lock() {
            for reading in &readings {
                if let SensorValue::Numeric(v) = &reading.value {
                    ts.push(
                        &reading.key,
                        *v,
                        reading.format,
                        config.general.update_time,
                    );
                }
            }
        }

        std::thread::sleep(update_interval);
    }
}
