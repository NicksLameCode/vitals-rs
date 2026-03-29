<div align="center">

# vitals-rs

**A native Rust system monitor for the Linux desktop**

A ground-up rewrite of the popular [Vitals](https://github.com/corecoding/Vitals) GNOME Shell extension as a standalone GTK4/Adwaita application with a D-Bus daemon and optional shell extension.

[![License: BSD-3-Clause](https://img.shields.io/badge/license-BSD--3--Clause-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021_edition-orange.svg)](https://www.rust-lang.org/)
[![GTK4](https://img.shields.io/badge/GTK4-Adwaita-4a86cf.svg)](https://gtk-rs.org/)
[![Tests](https://img.shields.io/badge/tests-105_passing-brightgreen.svg)]()

</div>

---

<!-- Screenshot will go here once available -->
<!-- <p align="center"><img src="data/screenshots/vitals-rs-screenshot.png" alt="Vitals screenshot" width="720" /></p> -->

## Features

- **10 sensor categories** -- temperature, voltage, fan, memory, processor, system, network, storage, battery, and GPU
- **GPU monitoring** -- NVIDIA via `nvidia-smi` subprocess, AMD and Intel via DRM sysfs
- **GTK4 / Adwaita** native desktop application with preferences dialog
- **D-Bus daemon** for headless servers, scripting, and extension integration
- **GNOME Shell extension** -- a thin ~215-line JavaScript client that reads sensor data from the daemon over D-Bus
- **Time-series history** with Cairo-rendered graphs, persisted across sessions
- **105 unit tests** with deterministic parsing tests for every sensor module
- **20 language translations** via gettext (`.po` files)
- **TOML configuration** at `~/.config/vitals/config.toml`
- **Flatpak-ready** build manifest included (GNOME Platform 47)
- **Release-optimized** with LTO and symbol stripping

## Architecture

vitals-rs is organized as a Cargo workspace with three crates:

```
vitals-core          (library -- sensor providers, config, formatting, history)
    |
    +--- vitals-app      (binary -- GTK4/Adwaita desktop application)
    |
    +--- vitals-daemon   (binary -- D-Bus service, headless polling)
              |
              +--- extension.js   (GNOME Shell extension, reads D-Bus)
```

| Crate | Type | Description |
|-------|------|-------------|
| `vitals-core` | Library | Sensor discovery and polling, hwmon parsing, config, unit formatting, time-series history |
| `vitals-app` | Binary | GTK4/libadwaita UI with sensor groups, live graphs, and a preferences dialog |
| `vitals-daemon` | Binary | Tokio + zbus D-Bus service exposing `com.corecoding.Vitals.Sensors` on the session bus |

The **`SensorProvider`** trait is the central abstraction. Each sensor category implements `discover()` for one-time hardware enumeration and `query(dwell)` for periodic polling. A `SensorManager` coordinates all enabled providers based on the loaded `AppConfig`.

## Installation

### Dependencies

Install the build-time and runtime dependencies for your distribution.

**Fedora / RHEL:**

```bash
sudo dnf install gtk4-devel libadwaita-devel gettext-devel meson ninja-build cargo
```

**Ubuntu / Debian:**

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev gettext meson ninja-build cargo
```

**Arch Linux:**

```bash
sudo pacman -S gtk4 libadwaita gettext meson ninja cargo
```

### Build from source

```bash
git clone https://github.com/NicksLameCode/vitals-rs.git
cd vitals-rs
cargo build --workspace --release
```

Binaries are placed in `target/release/vitals-app` and `target/release/vitals-daemon`.

### System install (meson + ninja)

```bash
meson setup builddir --prefix=/usr
ninja -C builddir
sudo ninja -C builddir install
```

This installs binaries to `/usr/bin/`, desktop files, icons, gschema XML, and translations.

### Flatpak (future)

A Flatpak manifest is included at `build-aux/flatpak/com.corecoding.Vitals.json` targeting the GNOME 47 runtime. Building:

```bash
flatpak-builder --user --install builddir build-aux/flatpak/com.corecoding.Vitals.json
```

## Usage

### Desktop application

```bash
vitals-app
```

Or launch "Vitals" from your application menu after a system install.

### D-Bus daemon

```bash
vitals-daemon
```

The daemon registers `com.corecoding.Vitals` on the session bus and exposes the `com.corecoding.Vitals.Sensors` interface at `/com/corecoding/Vitals`. It polls sensors at the configured interval and serves readings over D-Bus.

You can query the daemon from the command line:

```bash
# Get all numeric readings
busctl --user call com.corecoding.Vitals /com/corecoding/Vitals \
    com.corecoding.Vitals.Sensors GetReadings

# Get time-series data for a sensor
busctl --user call com.corecoding.Vitals /com/corecoding/Vitals \
    com.corecoding.Vitals.Sensors GetTimeSeries s "_memory_usage_"
```

### GNOME Shell extension

The optional GNOME Shell extension lives in `extension/` and acts as a thin D-Bus client (~215 lines). It requires the daemon to be running.

```bash
# Copy to GNOME Shell extensions directory
mkdir -p ~/.local/share/gnome-shell/extensions/Vitals@CoreCoding.com
cp extension/* ~/.local/share/gnome-shell/extensions/Vitals@CoreCoding.com/

# Start the daemon (or set it up as a systemd user service)
vitals-daemon &

# Restart GNOME Shell (X11) or log out and back in (Wayland)
```

Supports GNOME Shell versions 45 through 50.

### Configuration

Configuration is stored at:

```
~/.config/vitals/config.toml
```

If the file does not exist, defaults are used. The configuration is created automatically when you change settings in the preferences dialog or through the D-Bus `SetConfig` method.

## Sensor Coverage

| Category | Data Source | Readings |
|----------|------------|----------|
| **Temperature** | `/sys/class/hwmon/*/temp*_input` | CPU, GPU, chipset, drive temps |
| **Voltage** | `/sys/class/hwmon/*/in*_input` | CPU Vcore, DIMM, 3.3V, 5V, 12V rails |
| **Fan** | `/sys/class/hwmon/*/fan*_input` | Fan RPM for each detected fan |
| **Memory** | `/proc/meminfo` | Usage %, used/free/cached/swap, physical total |
| **Processor** | `/proc/stat`, `/proc/cpuinfo`, `/sys/devices/system/cpu/*/cpufreq/` | Per-core usage %, frequencies, model name |
| **System** | `/proc/uptime`, `/proc/loadavg`, `/proc/sys/fs/file-nr`, `/etc/hostname` | Uptime, load averages, open file count, hostname, kernel |
| **Network** | `/sys/class/net/*/statistics/`, `/proc/net/wireless` | TX/RX speeds per interface, WiFi quality, public IP |
| **Storage** | `/proc/mounts`, `/proc/diskstats`, `/proc/spl/kstat/zfs/arcstats` | Disk usage, read/write rates, ZFS ARC stats |
| **Battery** | `/sys/class/power_supply/BAT*/uevent` | Charge %, state, voltage, current, time remaining, cycles |
| **GPU** | `nvidia-smi` (NVIDIA) or `/sys/class/drm/card*/` (AMD/Intel) | Utilization, temp, VRAM, power, clocks, PCIe link |

## Configuration

The TOML configuration file supports the following sections:

```toml
[general]
update_time = 5              # Seconds between polls (1-60)
use_higher_precision = false # Extra decimal digit
alphabetize = true           # Sort sensors alphabetically
hide_zeros = false           # Hide sensors reporting zero
fixed_widths = true          # Prevent UI jitter
monitor_cmd = "gnome-system-monitor"

[temperature]
show = true
unit = 0                     # 0 = Celsius, 1 = Fahrenheit

[voltage]
show = true

[fan]
show = true

[memory]
show = true
measurement = 1              # 0 = binary (GiB), 1 = decimal (GB)

[processor]
show = true
include_static_info = false  # Include CPU model name

[system]
show = true

[network]
show = true
include_public_ip = true
speed_format = 0             # 0 = bytes/s, 1 = bits/s

[storage]
show = true
path = "/"
measurement = 1              # 0 = binary (GiB), 1 = decimal (GB)

[battery]
show = false
slot = 0                     # Battery slot index (0-7)

[gpu]
show = false
include_static_info = false

[history]
show_graphs = true
duration_seconds = 3600      # Seconds of history to keep

# Sensors pinned to the panel / header
hot_sensors = [
    "_memory_usage_",
    "_system_load_1m_",
    "__network-rx_max__",
]
```

## Development

### Running tests

```bash
cargo test --workspace
```

All 105 tests are deterministic -- sensor parsing functions accept string contents rather than reading from the filesystem, making them runnable in any environment including CI.

### Project structure

```
vitals-rs/
+-- Cargo.toml                     Workspace root
+-- meson.build                    Meson build for system install
+-- crates/
|   +-- vitals-core/
|   |   +-- src/
|   |       +-- lib.rs             Public modules
|   |       +-- config.rs          AppConfig (TOML serde)
|   |       +-- format.rs          Unit formatting (legible values)
|   |       +-- history.rs         TimeSeriesStore (JSON persistence)
|   |       +-- hwmon.rs           /sys/class/hwmon discovery
|   |       +-- sensors/
|   |           +-- mod.rs         SensorProvider trait, SensorManager
|   |           +-- temperature.rs
|   |           +-- voltage.rs
|   |           +-- fan.rs
|   |           +-- memory.rs
|   |           +-- processor.rs
|   |           +-- system.rs
|   |           +-- network.rs
|   |           +-- storage.rs
|   |           +-- battery.rs
|   |           +-- gpu/
|   |               +-- mod.rs     GpuProvider coordinator
|   |               +-- nvidia.rs  nvidia-smi subprocess
|   |               +-- amd.rs     AMD-specific DRM parsing
|   |               +-- drm.rs     Generic DRM sysfs discovery
|   +-- vitals-app/
|   |   +-- src/
|   |       +-- main.rs
|   |       +-- app.rs
|   |       +-- window.rs
|   |       +-- widgets/
|   |           +-- mod.rs
|   |           +-- sensor_row.rs
|   |           +-- sensor_group.rs
|   |           +-- history_graph.rs
|   |           +-- preferences.rs
|   +-- vitals-daemon/
|       +-- src/
|           +-- main.rs            Polling loop
|           +-- dbus.rs            zbus interface
+-- extension/
|   +-- extension.js               GNOME Shell D-Bus client
|   +-- metadata.json
|   +-- stylesheet.css
+-- data/
|   +-- com.corecoding.Vitals.desktop
|   +-- com.corecoding.Vitals.metainfo.xml
|   +-- com.corecoding.Vitals.gschema.xml
+-- po/                            20 translation files (.po)
+-- build-aux/flatpak/             Flatpak build manifest
```

### Adding a new sensor provider

1. Create a new file in `crates/vitals-core/src/sensors/` (e.g., `bluetooth.rs`).

2. Implement the `SensorProvider` trait:

    ```rust
    use crate::sensors::{SensorCategory, SensorProvider, SensorReading};

    pub struct BluetoothProvider { /* ... */ }

    impl SensorProvider for BluetoothProvider {
        fn category(&self) -> SensorCategory {
            // Add a new variant to SensorCategory if needed
            SensorCategory::System
        }

        fn discover(&mut self) -> Vec<SensorReading> {
            // One-time device enumeration
            Vec::new()
        }

        fn query(&mut self, dwell: f64) -> Vec<SensorReading> {
            // Read current values
            Vec::new()
        }
    }
    ```

3. Add the module to `sensors/mod.rs` and register it in `SensorManager::new()`.

4. Add a corresponding config section in `config.rs` with a `show` field.

5. Write parsing tests with sample data strings (see existing tests for patterns).

### Linting and formatting

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

## Translations

vitals-rs supports **20 languages**: Arabic, Belarusian, Catalan, Chinese (Simplified), Czech, Dutch, English, Finnish, French, German, Italian, Burmese, Occitan, Polish, Portuguese, Portuguese (Brazilian), Russian, Slovak, Turkish, and Ukrainian.

Translation files are in the `po/` directory using standard gettext `.po` format. To contribute a translation:

1. Copy `po/vitals.pot` to `po/<lang_code>.po`.
2. Translate the strings using a tool like [Poedit](https://poedit.net/) or a text editor.
3. Add your language code to `po/LINGUAS`.
4. Submit a pull request.

See [CONTRIBUTING.md](CONTRIBUTING.md) for more details.

## Credits

vitals-rs is a Rust rewrite of [Vitals](https://github.com/corecoding/Vitals) by [corecoding](https://github.com/corecoding). The original project is a widely-used GNOME Shell extension written in GJS (JavaScript) that has been providing system monitoring to the GNOME desktop for years.

This rewrite aims to bring the same monitoring capabilities to a standalone native application while maintaining compatibility through the D-Bus interface and an optional lightweight shell extension.

## License

This project is licensed under the **BSD-3-Clause License**. See the [LICENSE](LICENSE) file for details.
