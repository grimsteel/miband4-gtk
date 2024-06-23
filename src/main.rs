use std::{env, error::Error, str::FromStr, time::Duration};

use band::MiBand;
use bluer::{Address, Session};
use chrono::Local;
use utils::decode_hex;

mod band;
mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let auth_key = env::var("BAND_AUTH_KEY").ok().and_then(|s| decode_hex(&s)).unwrap();
    
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
    println!("Battery: {:?}", battery_status);
    println!("Band time: {:?}", band.get_band_time().await?);
    println!("Firmware: {:?}", band.get_firmware_revision().await?);
    band.authenticate(&auth_key).await?;
    //println!("setting time");
    //band.set_band_time(Local::now()).await?;
    println!("Activity: {:?}", band.get_current_activity().await?);

    Ok(())
}
