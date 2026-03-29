mod app;
mod widgets;
mod window;

use gettextrs::LocaleCategory;
use gtk4 as gtk;

fn setup_i18n() {
    gettextrs::setlocale(LocaleCategory::LcAll, "");
    gettextrs::bindtextdomain("vitals", "/usr/share/locale").expect("Failed to bind text domain");
    gettextrs::textdomain("vitals").expect("Failed to set text domain");
}

fn main() {
    env_logger::init();
    setup_i18n();
    log::info!("Vitals app starting...");

    let application = app::VitalsApplication::new();
    application.run();
}
