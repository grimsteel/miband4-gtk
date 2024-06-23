use std::{env, error::Error, str::FromStr, time::Duration};

use band::MiBand;
use bluer::{Address, Session};

mod band;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let session = Session::new().await?;
    // get the adapter
    let adapter = session.default_adapter().await?;

    let found = MiBand::discover(&adapter, Duration::from_secs(50)).await?;
    let device = found.into_values().next().unwrap();

    //let mac_address = env::var("BAND_MAC").unwrap();
    //let device = adapter.device(Address::from_str(&mac_address).unwrap()).unwrap();

    let mut band = MiBand::new(device, None);
    band.initialize().await?;
    let battery_status = band.get_battery().await?;
    println!("Battery: {:?}", battery_status);
    println!("Band time: {:?}", band.get_band_time().await?);
    println!("Firmware: {:?}", band.get_firmware_revision().await?);
    /*let found = discover_mi_bands(&adapter, 50000).await?;
    let test_device = found.values().next().unwrap();
    println!("connecting to device {}", test_device.address());
    if !test_device.is_connected().await? {
        test_device.connect().await?;
    }
    println!("connected successfully, authenticating");
    //let auth_key = env::var("BAND_AUTH_KEY").unwrap();*/

    Ok(())
}
