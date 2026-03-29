<div align="center">

<br>

<img src="https://raw.githubusercontent.com/NicksLameCode/vitals-rs/main/data/icons/original/cpu-symbolic.svg" width="80" alt="Vitals">

# vitals-rs

### Your system's vital signs, at a glance.

A ground-up Rust rewrite of the popular [Vitals](https://github.com/corecoding/Vitals) GNOME Shell extension --
rebuilt as a native GTK4/Adwaita desktop app, D-Bus daemon, and optional shell extension.

<br>

[![License](https://img.shields.io/badge/License-BSD_3--Clause-blue?style=for-the-badge)](LICENSE)
&nbsp;
[![Rust](https://img.shields.io/badge/Rust-2024_Edition-f74c00?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org/)
&nbsp;
[![GTK4](https://img.shields.io/badge/GTK4-Adwaita-4a86cf?style=for-the-badge&logo=gnome&logoColor=white)](https://gtk-rs.org/)
&nbsp;
[![Tests](https://img.shields.io/badge/Tests-105_Passing-2ea44f?style=for-the-badge)]()

<br>

<!-- Replace with real screenshot when available -->
<!--
<img src="data/screenshots/vitals-rs-screenshot.png" alt="Vitals screenshot" width="720">
<br><br>
-->

[Features](#features) &#8226;
[Performance](#performance) &#8226;
[Installation](#installation) &#8226;
[Usage](#usage) &#8226;
[Configuration](#configuration) &#8226;
[Contributing](#development)

<br>

</div>

---

<br>

## Features

<table>
<tr>
<td width="50%" valign="top">

**Comprehensive Monitoring**
- 10 sensor categories out of the box
- Temperature, voltage, fan, memory, CPU, system, network, storage, battery, GPU
- NVIDIA via `nvidia-smi`, AMD/Intel via DRM sysfs
- Per-core CPU usage and frequency tracking
- ZFS ARC cache statistics

</td>
<td width="50%" valign="top">

**Modern Architecture**
- Pure Rust sensor library with zero GTK dependencies
- GTK4/Adwaita native desktop application
- D-Bus daemon for headless servers and scripting
- Thin GNOME Shell extension (~215 lines, D-Bus client only)
- Cargo workspace with clean separation of concerns

</td>
</tr>
<tr>
<td width="50%" valign="top">

**Production Ready**
- 105 unit tests with deterministic parsing
- 20 language translations via gettext
- TOML configuration with preferences UI
- Flatpak manifest included (GNOME Platform 47)
- Meson build system for distro packaging

</td>
<td width="50%" valign="top">

**Built for Performance**
- 4.8x less memory than the GJS original
- Zero garbage collection pauses
- Native threading for subprocess management
- LTO-optimized release binaries
- Time-series history with Cairo-rendered graphs

</td>
</tr>
</table>

<br>

---

<br>

## Performance

> Real benchmarks measured on the same machine (AMD Ryzen, 28 GiB RAM, Fedora 45, kernel 7.0).
> Reproduce with `cargo run --release --example benchmark`.

<br>

<table>
<tr>
<th width="25%" align="center">4.8x</th>
<th width="25%" align="center">2x</th>
<th width="25%" align="center">13%</th>
<th width="25%" align="center">0</th>
</tr>
<tr>
<td align="center"><strong>Less Memory</strong><br><sub>15.4 MiB vs 73.9 MiB RSS</sub></td>
<td align="center"><strong>Faster Formatting</strong><br><sub>0.105 us vs 0.203 us/call</sub></td>
<td align="center"><strong>Faster P99 Latency</strong><br><sub>26.9 ms vs 31.0 ms</sub></td>
<td align="center"><strong>GC Pauses</strong><br><sub>Ownership model, no runtime</sub></td>
</tr>
</table>

<br>

### Memory Usage

The Rust daemon uses **15.4 MiB RSS** while collecting all 10 sensor categories. The GJS extension consumes 35-74 MiB for its JavaScript runtime alone, before accounting for GNOME Shell overhead.

| | vitals-rs | Vitals (GJS) | Delta |
|:--|--:|--:|:--:|
| RSS (idle, after first poll) | **15.4 MiB** | 73.9 MiB | **4.8x less** |
| RSS (runtime baseline) | **15.4 MiB** | 34.7 MiB | **2.3x less** |

### Sensor Poll Latency

100 consecutive polls reading temperature, voltage, fan, memory, CPU, system, network, and storage from `/proc` and `/sys`:

| | vitals-rs | Vitals (GJS) |
|:--|--:|--:|
| Average | **17.7 ms** | 18.3 ms |
| Median | **17.1 ms** | 17.6 ms |
| P99 | **26.9 ms** | 31.0 ms |
| Max | **30.9 ms** | 36.1 ms |
| Readings/poll | **71** | -- |

Both are I/O-bound (kernel VFS reads), but Rust is consistently faster at the tail -- zero GC pauses mean no surprise latency spikes. The Rust numbers include **full parsing and computation**; the GJS numbers measure raw file reads only.

### Value Formatting

50,000 sensor values formatted (temperature, percent, hertz, memory, watt with unit scaling):

| | vitals-rs | Vitals (GJS) | Delta |
|:--|--:|--:|:--:|
| Total | **5.2 ms** | 10.2 ms | **2.0x faster** |
| Per call | **0.105 us** | 0.203 us | **1.9x faster** |

### Engineering Comparison

| | vitals-rs | Vitals (GJS) |
|:--|:--:|:--:|
| Type safety | Compile-time | Runtime |
| Concurrency | Native threads | Single-threaded |
| GC pauses | None | Periodic |
| Unit tests | **105** | 0 |
| Memory safety | Ownership model | Manual |
| Error handling | `Result` / `Option` | Exceptions |

<br>

---

<br>

## Architecture

```
                        +------------------+
                        |   vitals-core    |    Pure Rust library
                        |                  |    No GTK dependencies
                        |  Sensors, Config |
                        |  Format, History |
                        +--------+---------+
                                 |
                    +------------+------------+
                    |                         |
           +--------v--------+      +---------v--------+
           |   vitals-app    |      |  vitals-daemon   |
           |                 |      |                  |
           |  GTK4/Adwaita   |      |  D-Bus Service   |
           |  Desktop App    |      |  Headless Mode   |
           +-----------------+      +--------+---------+
                                             |
                                    +--------v--------+
                                    |  extension.js   |
                                    |                 |
                                    |  GNOME Shell    |
                                    |  Panel Client   |
                                    +-----------------+
```

| Crate | Description |
|:------|:------------|
| **vitals-core** | Sensor discovery and polling, hwmon parsing, TOML config, value formatting, time-series history with JSON persistence. Zero GUI dependencies. |
| **vitals-app** | GTK4/libadwaita desktop UI with collapsible sensor groups, hot sensors bar, Cairo history graphs, and a full preferences dialog. |
| **vitals-daemon** | Headless D-Bus service (`com.corecoding.Vitals.Sensors`) built on zbus. Polls sensors on a timer and serves readings to any D-Bus client. |
| **extension.js** | Thin GNOME Shell extension (~215 lines). Connects to the daemon over D-Bus, renders sensor values in the top panel. Zero file I/O. |

The **`SensorProvider`** trait is the central abstraction. Each sensor category implements `discover()` for one-time hardware enumeration and `query(dwell)` for periodic polling. A `SensorManager` coordinates all enabled providers based on the loaded `AppConfig`.

<br>

---

<br>

## Installation

### Dependencies

<details>
<summary><strong>Fedora / RHEL</strong></summary>

```bash
sudo dnf install gtk4-devel libadwaita-devel gettext-devel meson ninja-build cargo
```
</details>

<details>
<summary><strong>Ubuntu / Debian</strong></summary>

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev gettext meson ninja-build cargo
```
</details>

<details>
<summary><strong>Arch Linux</strong></summary>

```bash
sudo pacman -S gtk4 libadwaita gettext meson ninja cargo
```
</details>

### Build from source

```bash
git clone https://github.com/NicksLameCode/vitals-rs.git
cd vitals-rs
cargo build --workspace --release
```

Binaries are placed in `target/release/vitals-app` and `target/release/vitals-daemon`.

### System install

```bash
meson setup builddir --prefix=/usr
ninja -C builddir
sudo ninja -C builddir install
```

Installs binaries, desktop file, D-Bus service, GSettings schema, icons, and translations.

### Flatpak

A manifest is included at `build-aux/flatpak/com.corecoding.Vitals.json` targeting GNOME 47:

```bash
flatpak-builder --user --install builddir build-aux/flatpak/com.corecoding.Vitals.json
```

<br>

---

<br>

## Usage

### Desktop App

```bash
vitals-app
```

Or launch **Vitals** from your application menu after system install.

### D-Bus Daemon

```bash
vitals-daemon
```

Query it from the command line:

```bash
# Get all sensor readings
busctl --user call com.corecoding.Vitals /com/corecoding/Vitals \
    com.corecoding.Vitals.Sensors GetReadings

# Get time-series history for a sensor
busctl --user call com.corecoding.Vitals /com/corecoding/Vitals \
    com.corecoding.Vitals.Sensors GetTimeSeries s "_memory_usage_"
```

### GNOME Shell Extension

The optional GNOME Shell extension (supporting versions 45-50) requires the daemon to be running:

```bash
# Install the extension
mkdir -p ~/.local/share/gnome-shell/extensions/Vitals@CoreCoding.com
cp extension/* ~/.local/share/gnome-shell/extensions/Vitals@CoreCoding.com/

# Start the daemon
vitals-daemon &

# Restart GNOME Shell (X11) or re-login (Wayland)
```

<br>

---

<br>

## Sensor Coverage

| Category | Source | What it monitors |
|:---------|:-------|:-----------------|
| **Temperature** | `/sys/class/hwmon/*/temp*_input` | CPU package, per-core, GPU, chipset, NVMe, wireless adapter |
| **Voltage** | `/sys/class/hwmon/*/in*_input` | CPU Vcore, DIMM, 3.3V / 5V / 12V rails |
| **Fan** | `/sys/class/hwmon/*/fan*_input` | RPM for each detected fan |
| **Memory** | `/proc/meminfo` | Usage %, allocated, free, cached, swap usage and totals |
| **Processor** | `/proc/stat`, `cpufreq`, `/proc/cpuinfo` | Per-core usage %, avg/min/max frequency, vendor, cache |
| **System** | `/proc/loadavg`, `/proc/uptime` | Load averages, uptime, open files, threads, kernel version |
| **Network** | `/sys/class/net/*/statistics/` | Per-interface TX/RX speeds, WiFi quality, public IP |
| **Storage** | `/proc/diskstats`, `statvfs` | Disk usage, read/write rates, ZFS ARC stats |
| **Battery** | `/sys/class/power_supply/BAT*/uevent` | Charge %, state, voltage, power, time remaining, cycles |
| **GPU** | `nvidia-smi` or `/sys/class/drm/card*/` | Utilization, temperature, VRAM, power, clocks, PCIe link |

<br>

---

<br>

## Configuration

Config lives at `~/.config/vitals/config.toml`. Created automatically on first preferences save.

<details>
<summary><strong>Full configuration reference</strong></summary>

```toml
[general]
update_time = 5              # Seconds between polls (1-60)
use_higher_precision = false # Extra decimal digit
alphabetize = true           # Sort sensors alphabetically
hide_zeros = false           # Hide sensors reporting zero
fixed_widths = true          # Prevent UI jitter
hide_icons = false           # Hide icons in panel
menu_centered = false        # Center dropdown menu
icon_style = 0               # 0 = Original, 1 = GNOME
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
include_static_info = false

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
slot = 0                     # 0 = BAT0, 1 = BAT1, ... 7 = macsmc-battery

[gpu]
show = false
include_static_info = false

[history]
show_graphs = true
duration_seconds = 3600      # 1 hour of history

hot_sensors = [
    "_memory_usage_",
    "_system_load_1m_",
    "__network-rx_max__",
]
```
</details>

<br>

---

<br>

## Development

### Quick start

```bash
git clone https://github.com/NicksLameCode/vitals-rs.git
cd vitals-rs

cargo build --workspace          # Build all crates
cargo test --workspace           # Run 105 tests
cargo run -p vitals-app          # Launch the GTK4 app
cargo run -p vitals-daemon       # Start the D-Bus daemon
cargo run --release --example benchmark   # Run benchmarks
```

### Linting

```bash
cargo fmt --all
cargo clippy --workspace -- -D warnings
```

### Adding a new sensor

1. Create `crates/vitals-core/src/sensors/your_sensor.rs`
2. Implement the `SensorProvider` trait (`discover()` + `query()`)
3. Add a config section in `config.rs` with a `show` field
4. Register in `SensorManager::new()` in `sensors/mod.rs`
5. Write tests with sample data strings

See existing providers for patterns. All parsing functions accept `&str` input for deterministic testing.

### Project structure

```
vitals-rs/
  Cargo.toml                     Workspace root
  meson.build                    System install
  crates/
    vitals-core/                 Sensor library (no GTK)
      src/
        sensors/                 10 sensor providers
          gpu/                   NVIDIA + AMD + DRM
        config.rs                TOML configuration
        format.rs                Value formatting
        history.rs               Time-series storage
        hwmon.rs                 Hardware monitor discovery
      examples/
        benchmark.rs             Performance benchmarks
    vitals-app/                  GTK4/Adwaita desktop app
      src/widgets/               UI components
    vitals-daemon/               D-Bus service
  extension/                     GNOME Shell extension
  data/                          Desktop file, icons, schemas
  po/                            20 translations
  build-aux/flatpak/             Flatpak manifest
```

<br>

---

<br>

## Translations

vitals-rs ships with **20 translations**: Arabic, Belarusian, Catalan, Chinese (Simplified), Czech, Dutch, Finnish, French, German, Italian, Burmese, Occitan, Polish, Portuguese, Portuguese (Brazilian), Russian, Slovak, Turkish, and Ukrainian.

To add a new language:

1. Copy `po/vitals.pot` to `po/<lang>.po`
2. Translate with [Poedit](https://poedit.net/) or any text editor
3. Add your language code to `po/LINGUAS`
4. Submit a pull request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

<br>

---

<br>

## Credits

vitals-rs is a Rust rewrite of [**Vitals**](https://github.com/corecoding/Vitals) by [corecoding](https://github.com/corecoding) -- a widely-used GNOME Shell extension that has been providing system monitoring to the Linux desktop for years. This project brings the same monitoring capabilities to a standalone native application with improved performance and reliability.

## License

Licensed under the [BSD-3-Clause License](LICENSE).

<br>
