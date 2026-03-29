use gtk4 as gtk;
use gtk::prelude::*;
use libadwaita as adw;
use adw::prelude::*;

/// A row displaying a single sensor reading with label, value, and optional star toggle.
#[derive(Clone)]
pub struct SensorRowWidget {
    pub row: adw::ActionRow,
    value_label: gtk::Label,
    key: String,
}

impl SensorRowWidget {
    pub fn new(label: &str, value: &str, key: &str) -> Self {
        let row = adw::ActionRow::builder()
            .title(label)
            .build();

        let value_label = gtk::Label::new(Some(value));
        value_label.add_css_class("dim-label");
        value_label.set_halign(gtk::Align::End);
        value_label.set_valign(gtk::Align::Center);
        row.add_suffix(&value_label);

        // Star toggle button for pinning to hot sensors
        let star_btn = gtk::ToggleButton::new();
        star_btn.set_icon_name("starred-symbolic");
        star_btn.add_css_class("flat");
        star_btn.set_valign(gtk::Align::Center);
        row.add_suffix(&star_btn);

        Self {
            row,
            value_label,
            key: key.to_string(),
        }
    }

    pub fn update_value(&self, value: &str) {
        self.value_label.set_text(value);
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}
