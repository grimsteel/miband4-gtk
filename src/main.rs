use gtk::{gdk::Display, gio::resources_register_include, glib::ExitCode, prelude::*, style_context_add_provider_for_display, Application, CssProvider, STYLE_PROVIDER_PRIORITY_USER};
use ui::window::MiBandWindow;
use utils::APP_ID;

mod band;
mod utils;
mod bluez;
mod store;
mod ui;

fn main() -> ExitCode {
    resources_register_include!("resources.gresource").expect("failed to register resources");

    env_logger::init();
    
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_app| {
        let provider = CssProvider::new();
        provider.load_from_resource("/me/grimsteel/miband4-gtk/style.css");
        style_context_add_provider_for_display(
            &Display::default().expect("Could not connect to display"),
            &provider,
            // for some reason my styles aren't working so I have to use user priority...
            STYLE_PROVIDER_PRIORITY_USER,
        );
        
    });
    // connect a handler to the activate signal
    app.connect_activate(|app| {
        MiBandWindow::new(app).present();
    });
    app.run()
}
