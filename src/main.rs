use std::{env, time::Duration};

use band::MiBand;
use bluez::BluezSession;
use gtk::{glib::{spawn_future_local, ExitCode}, prelude::*, Application, ApplicationWindow, Box as GBox, Button, HeaderBar, Label, Orientation};
use utils::decode_hex;

mod band;
mod utils;
mod bluez;

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

const APP_ID: &'static str = "com.github.grimsteel.miband4-gtk";


fn main() -> ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    // connect a handler to the activate signal
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let auth_key = env::var("BAND_AUTH_KEY").ok().and_then(|s| decode_hex(&s)).unwrap();
    
    let button = Button::builder()
        .label("Start scan")
        .build();

    button.connect_clicked(|b| {
        b.set_label("Scanning...");
        b.set_sensitive(false);
    });

    let text_unpowered = Label::builder()
        .label("Bluetooth is off")
        .build();

    let g_box = GBox::builder()
        .orientation(Orientation::Vertical)
        .margin_bottom(16)
        .margin_top(16)
        .margin_start(16)
        .margin_end(16)
        .build();

    g_box.append(&button);
    g_box.append(&text_unpowered);

    let title = Label::builder()
        .label("Mi Band 4")
        .build();

    let titlebar = HeaderBar::builder()
        .title_widget(&title)
        .build();
    
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Mi Band 4")
        .child(&g_box)
        .titlebar(&titlebar)
        .build();
    
    window.present();

    spawn_future_local(async move {
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
    });
}
