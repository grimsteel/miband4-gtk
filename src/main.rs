use std::{env, time::Duration};

use band::MiBand;
use bluez::BluezSession;
use gtk::{gio::resources_register_include, glib::{spawn_future_local, ExitCode}, prelude::*, Application};
use ui::window::MiBandWindow;
use utils::decode_hex;

mod band;
mod utils;
mod bluez;
mod ui;

const APP_ID: &'static str = "me.grimsteel.miband4-gtk";


fn main() -> ExitCode {
    resources_register_include!("resources.gresource").expect("failed to register resources");
    
    let app = Application::builder().application_id(APP_ID).build();
    // connect a handler to the activate signal
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let window = MiBandWindow::new(app);
    window.present();

    /*spawn_future_local(async move {
        if let Ok(session) = BluezSession::new().await {
            let powered = session.adapter.powered().await.unwrap_or(false);
            button.set_visible(powered);
            text_unpowered.set_visible(!powered);

            if let Ok(all_bands) = MiBand::discover(session.clone(), Duration::from_secs(5)).await {
                let band = all_bands.first().unwrap();
                println!("found device {band:?}");
                if let Ok(mut band) = MiBand::from_discovered_device(session.clone(), band).await {
                    println!("{:?}", band.initialize().await);
                    println!("{:?}", band.authenticate(&auth_key).await);
                    
                }
            };
        };
    });*/
}
