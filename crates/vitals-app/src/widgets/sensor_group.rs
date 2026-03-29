use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;

use super::sensor_row::SensorRowWidget;

/// A collapsible group of sensor rows for a single category.
#[derive(Clone)]
pub struct SensorGroupWidget {
    pub expander: adw::ExpanderRow,
    rows: Rc<RefCell<HashMap<String, SensorRowWidget>>>,
}

impl SensorGroupWidget {
    pub fn new(title: &str) -> Self {
        let expander = adw::ExpanderRow::builder()
            .title(title)
            .show_enable_switch(false)
            .build();

        Self {
            expander,
            rows: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    /// Update an existing sensor row or add a new one.
    pub fn update_or_add_sensor(&self, label: &str, value: &str, key: &str) {
        let mut rows = self.rows.borrow_mut();

        if let Some(existing) = rows.get(key) {
            existing.update_value(value);
        } else {
            let row = SensorRowWidget::new(label, value, key);
            self.expander.add_row(&row.row);
            rows.insert(key.to_string(), row);
        }
    }

    /// Update the subtitle (summary value) shown in the group header.
    pub fn set_subtitle(&self, subtitle: &str) {
        self.expander.set_subtitle(subtitle);
    }
}
