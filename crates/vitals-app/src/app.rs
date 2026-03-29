use std::sync::Arc;

use gtk4 as gtk;
use libadwaita as adw;
use adw::prelude::*;

use vitals_core::config::AppConfig;

use crate::window::build_window;

pub struct VitalsApplication {
    app: adw::Application,
}

impl VitalsApplication {
    pub fn new() -> Self {
        let app = adw::Application::builder()
            .application_id("com.corecoding.Vitals")
            .build();

        app.connect_activate(move |app| {
            let config = Arc::new(AppConfig::load());
            let window = build_window(app, config);
            window.present();
        });

        Self { app }
    }

    pub fn run(&self) {
        self.app.run();
    }
}
