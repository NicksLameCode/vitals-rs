use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use vitals_core::config::AppConfig;
use vitals_core::format::ValueFormatter;
use vitals_core::history::TimeSeriesStore;
use vitals_core::sensors::{SensorManager, SensorValue};

use crate::widgets::preferences::show_preferences;
use crate::widgets::sensor_group::SensorGroupWidget;

pub fn build_window(app: &adw::Application, config: Arc<AppConfig>) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Vitals")
        .default_width(380)
        .default_height(600)
        .build();

    // Shared mutable config for preferences
    let shared_config: Rc<RefCell<AppConfig>> = Rc::new(RefCell::new((*config).clone()));

    let header_bar = adw::HeaderBar::new();

    // Refresh button
    let refresh_btn = gtk::Button::from_icon_name("view-refresh-symbolic");
    refresh_btn.set_tooltip_text(Some("Refresh"));
    header_bar.pack_start(&refresh_btn);

    // System monitor button
    let monitor_cmd = config.general.monitor_cmd.clone();
    let monitor_btn = gtk::Button::from_icon_name("utilities-system-monitor-symbolic");
    monitor_btn.set_tooltip_text(Some("System Monitor"));
    monitor_btn.connect_clicked(move |_| {
        let _ = std::process::Command::new(&monitor_cmd).spawn();
    });
    header_bar.pack_end(&monitor_btn);

    // Preferences button
    let prefs_btn = gtk::Button::from_icon_name("preferences-system-symbolic");
    prefs_btn.set_tooltip_text(Some("Preferences"));
    {
        let window_ref = window.clone();
        let prefs_config = shared_config.clone();
        prefs_btn.connect_clicked(move |_| {
            show_preferences(&window_ref, prefs_config.clone());
        });
    }
    header_bar.pack_end(&prefs_btn);

    // Main content
    let content_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content_box.append(&header_bar);

    // Hot sensors bar at top
    let hot_bar = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    hot_bar.set_margin_start(12);
    hot_bar.set_margin_end(12);
    hot_bar.set_margin_top(6);
    hot_bar.set_margin_bottom(6);
    hot_bar.add_css_class("hot-sensors-bar");
    content_box.append(&hot_bar);

    // Scrolled list of sensor groups
    let scrolled_window = gtk::ScrolledWindow::new();
    scrolled_window.set_vexpand(true);
    scrolled_window.set_hscrollbar_policy(gtk::PolicyType::Never);

    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::None);
    list_box.add_css_class("boxed-list");
    list_box.set_margin_start(12);
    list_box.set_margin_end(12);
    list_box.set_margin_top(6);
    list_box.set_margin_bottom(12);

    scrolled_window.set_child(Some(&list_box));
    content_box.append(&scrolled_window);

    window.set_content(Some(&content_box));

    // Sensor groups
    let groups = create_sensor_groups(&list_box, &config);

    // Set up polling timer
    let sensor_manager = Rc::new(RefCell::new(SensorManager::new(&config)));
    let time_series = Rc::new(RefCell::new(TimeSeriesStore::new(config.history.duration_seconds)));
    let last_query = Rc::new(RefCell::new(Instant::now()));
    let formatter_config = config.clone();
    let hot_sensors = config.hot_sensors.clone();

    // Load cached history
    let history_path = AppConfig::history_path();
    let _ = time_series.borrow_mut().load(&history_path);

    let update_time = config.general.update_time;

    // Clone the Rc before we move it into the timeout closure so we can also
    // use it in the close_request handler.
    let time_series_for_close = time_series.clone();

    glib::timeout_add_seconds_local(update_time, {
        let hot_bar = hot_bar.clone();
        let groups = groups.clone();
        let sensor_manager = sensor_manager.clone();
        let time_series = time_series.clone();
        let last_query = last_query.clone();
        move || {
            let now = Instant::now();
            let dwell = now
                .duration_since(*last_query.borrow())
                .as_secs_f64();
            *last_query.borrow_mut() = now;

            let readings = sensor_manager.borrow_mut().query_all(dwell);
            let formatter = ValueFormatter::new(&formatter_config);

            // Update sensor rows
            for reading in &readings {
                let cat_key = reading.category.to_string();
                if let Some(group) = groups.get(&cat_key) {
                    let formatted = match &reading.value {
                        SensorValue::Numeric(v) => formatter.format(*v, reading.format),
                        SensorValue::Text(t) => t.clone(),
                        SensorValue::Disabled => "disabled".to_string(),
                    };
                    group.update_or_add_sensor(&reading.label, &formatted, &reading.key);
                }

                // Update time series
                if let SensorValue::Numeric(v) = &reading.value {
                    time_series.borrow_mut().push(
                        &reading.key,
                        *v,
                        reading.format,
                        update_time,
                    );
                }
            }

            // Update hot sensors bar
            update_hot_bar(&hot_bar, &readings, &hot_sensors, &formatter_config);

            glib::ControlFlow::Continue
        }
    });

    // Save history on window close
    window.connect_close_request(move |_| {
        let history_path = AppConfig::history_path();
        if let Err(e) = time_series_for_close.borrow().save(&history_path) {
            log::error!("Failed to save history: {e}");
        }
        glib::Propagation::Proceed
    });

    window
}

fn create_sensor_groups(
    list_box: &gtk::ListBox,
    config: &AppConfig,
) -> HashMap<String, SensorGroupWidget> {
    let mut groups = HashMap::new();
    let categories = [
        ("temperature", "Temperature", config.temperature.show),
        ("voltage", "Voltage", config.voltage.show),
        ("fan", "Fan", config.fan.show),
        ("memory", "Memory", config.memory.show),
        ("processor", "Processor", config.processor.show),
        ("system", "System", config.system.show),
        ("network", "Network", config.network.show),
        ("storage", "Storage", config.storage.show),
        ("battery", "Battery", config.battery.show),
    ];

    for (key, title, show) in categories {
        if show {
            let group = SensorGroupWidget::new(title);
            list_box.append(&group.expander);
            groups.insert(key.to_string(), group);
        }
    }

    // GPU groups (up to 4 GPUs)
    if config.gpu.show {
        for i in 1..=4u8 {
            let key = format!("gpu#{i}");
            let title = format!("GPU {i}");
            let group = SensorGroupWidget::new(&title);
            list_box.append(&group.expander);
            groups.insert(key, group);
        }
    }

    groups
}

fn update_hot_bar(
    hot_bar: &gtk::Box,
    readings: &[vitals_core::sensors::SensorReading],
    hot_sensors: &[String],
    config: &AppConfig,
) {
    // Remove existing children
    while let Some(child) = hot_bar.first_child() {
        hot_bar.remove(&child);
    }

    let formatter = ValueFormatter::new(config);

    for hot_key in hot_sensors {
        if let Some(reading) = readings.iter().find(|r| r.key == *hot_key) {
            let formatted = match &reading.value {
                SensorValue::Numeric(v) => formatter.format(*v, reading.format),
                SensorValue::Text(t) => t.clone(),
                SensorValue::Disabled => continue,
            };

            let label = gtk::Label::new(Some(&format!("{}: {}", reading.label, formatted)));
            label.add_css_class("dim-label");
            hot_bar.append(&label);
        }
    }
}
