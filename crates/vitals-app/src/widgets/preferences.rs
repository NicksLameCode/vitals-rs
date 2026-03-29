use std::cell::RefCell;
use std::rc::Rc;

use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

use vitals_core::config::AppConfig;

/// Build and show the preferences window. When closed, all UI values are
/// written back into the shared `AppConfig` and persisted to disk.
pub fn show_preferences(parent: &adw::ApplicationWindow, config: Rc<RefCell<AppConfig>>) {
    let prefs_window = adw::PreferencesWindow::builder()
        .title("Vitals Preferences")
        .transient_for(parent)
        .modal(true)
        .build();

    // Snapshot the current config so we can populate widgets.
    let cfg = config.borrow().clone();

    // ── General page ────────────────────────────────────────────────────
    let general_page = adw::PreferencesPage::builder()
        .title("General")
        .icon_name("preferences-system-symbolic")
        .build();

    let general_group = adw::PreferencesGroup::builder()
        .title("General Settings")
        .build();

    // Update interval
    let update_row = adw::SpinRow::builder()
        .title("Update Interval")
        .subtitle("Seconds between sensor polls")
        .adjustment(&gtk::Adjustment::new(
            cfg.general.update_time as f64,
            1.0,
            60.0,
            1.0,
            5.0,
            0.0,
        ))
        .build();
    general_group.add(&update_row);

    // Higher precision
    let precision_row = adw::SwitchRow::builder()
        .title("Higher Precision")
        .subtitle("Show extra decimal places")
        .active(cfg.general.use_higher_precision)
        .build();
    general_group.add(&precision_row);

    // Alphabetize
    let alpha_row = adw::SwitchRow::builder()
        .title("Alphabetize Sensors")
        .active(cfg.general.alphabetize)
        .build();
    general_group.add(&alpha_row);

    // Hide zeros
    let zeros_row = adw::SwitchRow::builder()
        .title("Hide Zero Values")
        .active(cfg.general.hide_zeros)
        .build();
    general_group.add(&zeros_row);

    // Fixed widths
    let fixed_row = adw::SwitchRow::builder()
        .title("Fixed Widths")
        .subtitle("Prevent UI jitter from changing values")
        .active(cfg.general.fixed_widths)
        .build();
    general_group.add(&fixed_row);

    // Hide icons
    let hide_icons_row = adw::SwitchRow::builder()
        .title("Hide Icons")
        .subtitle("Show only sensor values")
        .active(cfg.general.hide_icons)
        .build();
    general_group.add(&hide_icons_row);

    // Menu centered
    let menu_centered_row = adw::SwitchRow::builder()
        .title("Center Menu")
        .active(cfg.general.menu_centered)
        .build();
    general_group.add(&menu_centered_row);

    // Icon style
    let icon_style_row = adw::ComboRow::builder()
        .title("Icon Style")
        .build();
    let icon_styles = gtk::StringList::new(&["Original", "GNOME"]);
    icon_style_row.set_model(Some(&icon_styles));
    icon_style_row.set_selected(cfg.general.icon_style);
    general_group.add(&icon_style_row);

    // Monitor command
    let monitor_row = adw::EntryRow::builder()
        .title("System Monitor Command")
        .text(&cfg.general.monitor_cmd)
        .build();
    general_group.add(&monitor_row);

    general_page.add(&general_group);
    prefs_window.add(&general_page);

    // ── Sensors page ────────────────────────────────────────────────────
    let sensors_page = adw::PreferencesPage::builder()
        .title("Sensors")
        .icon_name("dialog-information-symbolic")
        .build();

    // Sensor enable/disable toggles
    let sensors_group = adw::PreferencesGroup::builder()
        .title("Enabled Sensors")
        .build();

    let temp_show_row = adw::SwitchRow::builder()
        .title("Temperature")
        .active(cfg.temperature.show)
        .build();
    sensors_group.add(&temp_show_row);

    let voltage_show_row = adw::SwitchRow::builder()
        .title("Voltage")
        .active(cfg.voltage.show)
        .build();
    sensors_group.add(&voltage_show_row);

    let fan_show_row = adw::SwitchRow::builder()
        .title("Fan")
        .active(cfg.fan.show)
        .build();
    sensors_group.add(&fan_show_row);

    let mem_show_row = adw::SwitchRow::builder()
        .title("Memory")
        .active(cfg.memory.show)
        .build();
    sensors_group.add(&mem_show_row);

    let proc_show_row = adw::SwitchRow::builder()
        .title("Processor")
        .active(cfg.processor.show)
        .build();
    sensors_group.add(&proc_show_row);

    let sys_show_row = adw::SwitchRow::builder()
        .title("System")
        .active(cfg.system.show)
        .build();
    sensors_group.add(&sys_show_row);

    let net_show_row = adw::SwitchRow::builder()
        .title("Network")
        .active(cfg.network.show)
        .build();
    sensors_group.add(&net_show_row);

    let storage_show_row = adw::SwitchRow::builder()
        .title("Storage")
        .active(cfg.storage.show)
        .build();
    sensors_group.add(&storage_show_row);

    let battery_show_row = adw::SwitchRow::builder()
        .title("Battery")
        .active(cfg.battery.show)
        .build();
    sensors_group.add(&battery_show_row);

    let gpu_show_row = adw::SwitchRow::builder()
        .title("GPU")
        .active(cfg.gpu.show)
        .build();
    sensors_group.add(&gpu_show_row);

    sensors_page.add(&sensors_group);

    // ── Temperature settings ────────────────────────────────────────────
    let temp_group = adw::PreferencesGroup::builder()
        .title("Temperature")
        .build();

    let temp_unit_row = adw::ComboRow::builder()
        .title("Temperature Unit")
        .build();
    let temp_units = gtk::StringList::new(&["Celsius", "Fahrenheit"]);
    temp_unit_row.set_model(Some(&temp_units));
    temp_unit_row.set_selected(cfg.temperature.unit);
    temp_group.add(&temp_unit_row);

    sensors_page.add(&temp_group);

    // ── Memory settings ─────────────────────────────────────────────────
    let mem_group = adw::PreferencesGroup::builder()
        .title("Memory")
        .build();

    let mem_measurement_row = adw::ComboRow::builder()
        .title("Measurement")
        .build();
    let mem_measurements = gtk::StringList::new(&["Binary (GiB)", "Decimal (GB)"]);
    mem_measurement_row.set_model(Some(&mem_measurements));
    mem_measurement_row.set_selected(cfg.memory.measurement);
    mem_group.add(&mem_measurement_row);

    sensors_page.add(&mem_group);

    // ── Processor settings ──────────────────────────────────────────────
    let proc_group = adw::PreferencesGroup::builder()
        .title("Processor")
        .build();

    let proc_static_row = adw::SwitchRow::builder()
        .title("Include Static Info")
        .active(cfg.processor.include_static_info)
        .build();
    proc_group.add(&proc_static_row);

    sensors_page.add(&proc_group);

    // ── Network settings ────────────────────────────────────────────────
    let net_group = adw::PreferencesGroup::builder()
        .title("Network")
        .build();

    let net_ip_row = adw::SwitchRow::builder()
        .title("Include Public IP")
        .active(cfg.network.include_public_ip)
        .build();
    net_group.add(&net_ip_row);

    let net_speed_row = adw::ComboRow::builder()
        .title("Speed Format")
        .build();
    let speed_units = gtk::StringList::new(&["Bytes/s", "Bits/s"]);
    net_speed_row.set_model(Some(&speed_units));
    net_speed_row.set_selected(cfg.network.speed_format);
    net_group.add(&net_speed_row);

    sensors_page.add(&net_group);

    // ── Storage settings ────────────────────────────────────────────────
    let storage_group = adw::PreferencesGroup::builder()
        .title("Storage")
        .build();

    let storage_path_row = adw::EntryRow::builder()
        .title("Mount Path")
        .text(&cfg.storage.path)
        .build();
    storage_group.add(&storage_path_row);

    let storage_measurement_row = adw::ComboRow::builder()
        .title("Measurement")
        .build();
    let storage_measurements = gtk::StringList::new(&["Binary (GiB)", "Decimal (GB)"]);
    storage_measurement_row.set_model(Some(&storage_measurements));
    storage_measurement_row.set_selected(cfg.storage.measurement);
    storage_group.add(&storage_measurement_row);

    sensors_page.add(&storage_group);

    // ── Battery settings ────────────────────────────────────────────────
    let battery_group = adw::PreferencesGroup::builder()
        .title("Battery")
        .build();

    let battery_slot_row = adw::SpinRow::builder()
        .title("Battery Slot")
        .subtitle("Slot index 0-7")
        .adjustment(&gtk::Adjustment::new(
            cfg.battery.slot as f64,
            0.0,
            7.0,
            1.0,
            1.0,
            0.0,
        ))
        .build();
    battery_group.add(&battery_slot_row);

    sensors_page.add(&battery_group);

    // ── GPU settings ────────────────────────────────────────────────────
    let gpu_group = adw::PreferencesGroup::builder()
        .title("GPU")
        .build();

    let gpu_static_row = adw::SwitchRow::builder()
        .title("Include Static Info")
        .active(cfg.gpu.include_static_info)
        .build();
    gpu_group.add(&gpu_static_row);

    sensors_page.add(&gpu_group);

    prefs_window.add(&sensors_page);

    // ── History page ────────────────────────────────────────────────────
    let history_page = adw::PreferencesPage::builder()
        .title("History")
        .icon_name("document-open-recent-symbolic")
        .build();

    let history_group = adw::PreferencesGroup::builder()
        .title("History Settings")
        .build();

    let history_graphs_row = adw::SwitchRow::builder()
        .title("Show Graphs")
        .active(cfg.history.show_graphs)
        .build();
    history_group.add(&history_graphs_row);

    let history_duration_row = adw::SpinRow::builder()
        .title("Duration (seconds)")
        .subtitle("How many seconds of history to keep")
        .adjustment(&gtk::Adjustment::new(
            cfg.history.duration_seconds as f64,
            60.0,
            86400.0,
            60.0,
            300.0,
            0.0,
        ))
        .build();
    history_group.add(&history_duration_row);

    history_page.add(&history_group);
    prefs_window.add(&history_page);

    // ── Save on close ───────────────────────────────────────────────────
    prefs_window.connect_close_request(move |_| {
        let mut cfg = config.borrow_mut();

        // General
        cfg.general.update_time = update_row.value() as u32;
        cfg.general.use_higher_precision = precision_row.is_active();
        cfg.general.alphabetize = alpha_row.is_active();
        cfg.general.hide_zeros = zeros_row.is_active();
        cfg.general.fixed_widths = fixed_row.is_active();
        cfg.general.hide_icons = hide_icons_row.is_active();
        cfg.general.menu_centered = menu_centered_row.is_active();
        cfg.general.icon_style = icon_style_row.selected();
        cfg.general.monitor_cmd = monitor_row.text().to_string();

        // Temperature
        cfg.temperature.show = temp_show_row.is_active();
        cfg.temperature.unit = temp_unit_row.selected();

        // Voltage
        cfg.voltage.show = voltage_show_row.is_active();

        // Fan
        cfg.fan.show = fan_show_row.is_active();

        // Memory
        cfg.memory.show = mem_show_row.is_active();
        cfg.memory.measurement = mem_measurement_row.selected();

        // Processor
        cfg.processor.show = proc_show_row.is_active();
        cfg.processor.include_static_info = proc_static_row.is_active();

        // System
        cfg.system.show = sys_show_row.is_active();

        // Network
        cfg.network.show = net_show_row.is_active();
        cfg.network.include_public_ip = net_ip_row.is_active();
        cfg.network.speed_format = net_speed_row.selected();

        // Storage
        cfg.storage.show = storage_show_row.is_active();
        cfg.storage.path = storage_path_row.text().to_string();
        cfg.storage.measurement = storage_measurement_row.selected();

        // Battery
        cfg.battery.show = battery_show_row.is_active();
        cfg.battery.slot = battery_slot_row.value() as u8;

        // GPU
        cfg.gpu.show = gpu_show_row.is_active();
        cfg.gpu.include_static_info = gpu_static_row.is_active();

        // History
        cfg.history.show_graphs = history_graphs_row.is_active();
        cfg.history.duration_seconds = history_duration_row.value() as u32;

        // Persist to disk
        if let Err(e) = cfg.save() {
            log::error!("Failed to save config: {e}");
        }

        glib::Propagation::Proceed
    });

    prefs_window.present();
}
