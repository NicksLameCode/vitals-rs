# Contributing to vitals-rs

Thank you for your interest in contributing to vitals-rs. This document covers how to set up your development environment, write code that fits the project's standards, and submit changes.

## Table of Contents

- [Development Environment Setup](#development-environment-setup)
- [Code Style](#code-style)
- [Adding a New Sensor Provider](#adding-a-new-sensor-provider)
- [Writing Tests](#writing-tests)
- [Adding Translations](#adding-translations)
- [Pull Request Guidelines](#pull-request-guidelines)

---

## Development Environment Setup

### Prerequisites

You need a Linux system with the following installed:

- **Rust** (stable, 2021 edition) -- install via [rustup](https://rustup.rs/)
- **GTK4 development libraries**
- **libadwaita development libraries**
- **gettext** (for translations)
- **meson and ninja** (for system install, optional for development)

### Installing dependencies

**Fedora / RHEL:**

```bash
sudo dnf install gtk4-devel libadwaita-devel gettext-devel meson ninja-build
```

**Ubuntu / Debian:**

```bash
sudo apt install libgtk-4-dev libadwaita-1-dev gettext meson ninja-build
```

**Arch Linux:**

```bash
sudo pacman -S gtk4 libadwaita gettext meson ninja
```

### Building and running

```bash
# Clone the repository
git clone https://github.com/NicksLameCode/vitals-rs.git
cd vitals-rs

# Build the workspace
cargo build --workspace

# Run the app
cargo run -p vitals-app

# Run the daemon
cargo run -p vitals-daemon

# Run all tests
cargo test --workspace
```

### Useful environment variables

```bash
# Enable debug logging
RUST_LOG=debug cargo run -p vitals-daemon

# Enable trace-level logging for a specific module
RUST_LOG=vitals_core::sensors::gpu=trace cargo run -p vitals-app
```

---

## Code Style

### Formatting

All code must be formatted with `rustfmt`. Run before committing:

```bash
cargo fmt --all
```

### Linting

All code must pass `clippy` without warnings:

```bash
cargo clippy --workspace -- -D warnings
```

### General guidelines

- **Error handling**: Use `anyhow::Result` for fallible functions. Avoid `.unwrap()` in production code; use `.unwrap()` only in tests.
- **Logging**: Use the `log` crate macros (`log::info!`, `log::warn!`, `log::error!`). Avoid `println!` and `eprintln!`.
- **Naming**: Follow Rust naming conventions. Sensor keys use the format `_category_name_` with underscores (e.g., `_memory_usage_`).
- **Dependencies**: Minimize new dependencies. If a feature can be accomplished with a few lines of code, prefer that over adding a crate.
- **Unsafe code**: Avoid `unsafe`. The project currently has zero unsafe blocks.
- **Comments**: Add doc comments (`///`) to all public types, traits, and functions. Use inline comments sparingly and only when the code's intent is non-obvious.

---

## Adding a New Sensor Provider

The sensor system is built around the `SensorProvider` trait defined in `crates/vitals-core/src/sensors/mod.rs`. Here is a step-by-step guide to adding a new provider.

### Step 1: Create the module file

Create a new file in `crates/vitals-core/src/sensors/`. For example, `bluetooth.rs`:

```rust
use crate::sensors::{
    SensorCategory, SensorFormat, SensorProvider, SensorReading, SensorValue,
};

pub struct BluetoothProvider {
    devices: Vec<String>,
}

impl BluetoothProvider {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
        }
    }

    /// Parse raw data into sensor readings.
    ///
    /// Accept a string parameter so the function can be tested
    /// without accessing the filesystem.
    pub fn parse_data(contents: &str) -> Vec<SensorReading> {
        let mut readings = Vec::new();
        // Parsing logic here...
        readings
    }
}

impl SensorProvider for BluetoothProvider {
    fn category(&self) -> SensorCategory {
        SensorCategory::System // or add a new variant
    }

    fn discover(&mut self) -> Vec<SensorReading> {
        // One-time device enumeration
        Vec::new()
    }

    fn query(&mut self, _dwell: f64) -> Vec<SensorReading> {
        // Read current values from the filesystem and call parse_data()
        Vec::new()
    }
}
```

### Step 2: Register the module

Add the module to `crates/vitals-core/src/sensors/mod.rs`:

```rust
pub mod bluetooth;
```

### Step 3: Add configuration

Add a config struct in `crates/vitals-core/src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BluetoothConfig {
    pub show: bool,
}

impl Default for BluetoothConfig {
    fn default() -> Self {
        Self { show: false }
    }
}
```

Add the field to `AppConfig`:

```rust
#[serde(default)]
pub bluetooth: BluetoothConfig,
```

### Step 4: Wire it into SensorManager

In the `SensorManager::new()` function in `sensors/mod.rs`, add:

```rust
if config.bluetooth.show {
    providers.push(Box::new(bluetooth::BluetoothProvider::new()));
}
```

### Step 5: Write tests

See the [Writing Tests](#writing-tests) section below.

---

## Writing Tests

The project follows a specific testing pattern that makes tests deterministic and environment-independent.

### The pattern

Each sensor module exposes a public `parse_*` function that accepts a `&str` of file contents and returns `Vec<SensorReading>`. Tests call these functions directly with known input data, avoiding any filesystem access.

### Example test

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_data_normal() {
        let contents = "key1=value1\nkey2=42\n";
        let readings = BluetoothProvider::parse_data(contents);

        assert_eq!(readings.len(), 2);
        assert_eq!(readings[0].label, "expected label");
        assert_eq!(readings[0].category, SensorCategory::System);
    }

    #[test]
    fn test_parse_data_empty() {
        let readings = BluetoothProvider::parse_data("");
        assert!(readings.is_empty());
    }

    #[test]
    fn test_parse_data_malformed() {
        // Verify graceful handling of unexpected input
        let contents = "garbage\n\n\x00binary";
        let readings = BluetoothProvider::parse_data(contents);
        // Should not panic
    }
}
```

### What to test

- **Normal input**: Realistic data from the actual source files (`/proc/*`, `/sys/*`, etc.)
- **Empty input**: Ensure empty strings produce empty results, not panics
- **Malformed input**: Truncated lines, missing fields, non-numeric values
- **Edge cases**: Zero values, negative numbers, very large numbers
- **Multiple entries**: Files with several devices/sensors

### Running tests

```bash
# All tests
cargo test --workspace

# Tests for a specific crate
cargo test -p vitals-core

# A single test by name
cargo test -p vitals-core test_parse_meminfo

# With output shown
cargo test --workspace -- --nocapture
```

---

## Adding Translations

vitals-rs uses gettext for internationalization. Translation files are in the `po/` directory.

### Adding a new language

1. **Copy the template**: Copy `po/vitals.pot` to `po/<language_code>.po` (e.g., `po/ja.po` for Japanese).

2. **Translate the strings**: Open the `.po` file in a text editor or a tool like [Poedit](https://poedit.net/) and translate each `msgstr` entry.

3. **Register the language**: Add your language code to `po/LINGUAS` (one code per line, sorted alphabetically).

4. **Test**: Build with meson to verify the translations compile:

    ```bash
    meson setup builddir
    ninja -C builddir
    ```

### Updating existing translations

If new translatable strings have been added to the source code:

1. Regenerate the template: The `POTFILES.in` file lists all source files containing translatable strings.

2. Use `msgmerge` to update your `.po` file:

    ```bash
    msgmerge --update po/<lang>.po po/vitals.pot
    ```

3. Translate any new `msgstr` entries marked as fuzzy or empty.

### Translatable source files

The `po/POTFILES.in` file lists every source file that contains translatable strings. When adding new UI strings, make sure to:

- Use `gettext` macros in the Rust code
- Add the source file path to `po/POTFILES.in` if it is not already listed

---

## Pull Request Guidelines

### Before submitting

1. **Create a topic branch** from `main`:

    ```bash
    git checkout -b my-feature main
    ```

2. **Run the full check suite**:

    ```bash
    cargo fmt --all --check
    cargo clippy --workspace -- -D warnings
    cargo test --workspace
    ```

3. **Keep commits focused**: Each commit should represent a single logical change. Squash fixup commits before requesting review.

4. **Write meaningful commit messages**: Use the imperative mood (e.g., "Add bluetooth sensor provider", not "Added bluetooth sensor provider"). Include context about *why* the change was made, not just *what* changed.

### PR description

Include the following in your pull request description:

- **What** the change does (one or two sentences)
- **Why** it is needed
- **How** to test it (if not obvious from the test suite)
- Any **breaking changes** or configuration changes

### Review process

- All PRs require at least one review before merging.
- CI must pass (formatting, linting, and tests).
- Address review feedback with new commits rather than force-pushing, so the review history is preserved. Commits will be squashed on merge.

### What makes a good contribution

- Bug fixes with a test that reproduces the bug
- New sensor providers with comprehensive parsing tests
- Translation additions or improvements
- Documentation improvements
- Performance improvements backed by measurements

### Reporting issues

When filing a bug report, include:

- Your Linux distribution and version
- GNOME Shell version (if applicable)
- Desktop environment (GNOME, KDE, etc.)
- The output of `vitals-daemon` with `RUST_LOG=debug` enabled
- Steps to reproduce the issue
