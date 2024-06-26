use std::{env, time::Duration};

use band::MiBand;
use bluez::BluezSession;
use gtk::{gio::resources_register_include, glib::{spawn_future_local, ExitCode}, prelude::*, Application, ApplicationWindow, Box as GBox, Button, HeaderBar, Label, Orientation};
use ui::window::MiBandWindow;
use utils::decode_hex;

mod band;
mod utils;
mod bluez;
mod ui;

/*#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    
    let session = Session::new().await?;
    // get the adapter
    let adapter = session.default_adapter().await?;

    let found = MiBand::discover(&adapter, Duration::from_secs(50)).await?;
    let device = found.into_values().next().unwrap();

    //let mac_address = env::var("BAND_MAC").unwrap();
    //let device = adapter.device(Address::from_str(&mac_address).unwrap()).unwrap();

    let mut band = MiBand::new(device);
    band.initialize().await?;
    let battery_status = band.get_battery().await?;
    //println!("Battery: {:?}", battery_status);
    println!("Band time: {:?}", band.get_band_time().await?);
    //println!("Firmware: {:?}", band.get_firmware_revision().await?);
    //band.authenticate(&auth_key).await?;
    //println!("setting time");
    //band.set_band_time(Local::now()).await?;
    //println!("Activity: {:?}", band.get_current_activity().await?);
    //band.set_band_time(Local::now()).await?;

    Ok(())
}*/

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
