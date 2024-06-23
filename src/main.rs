use std::{collections::{HashMap, HashSet}, error::Error};

use bluer::{Adapter, AdapterEvent, Address, DiscoveryFilter, DiscoveryTransport, Session, Uuid};
use futures::{pin_mut, StreamExt};

const SERVICE_BAND_0: Uuid = match Uuid::try_parse("0000fee0-0000-1000-8000-00805f9b34fb") {
    Ok(u) => u,
    _ => panic!("bad uuid")
};

async fn discover_mi_bands(adapter: &Adapter) -> bluer::Result<HashMap<Address, String>> {
    let mut device_map = HashMap::new();
    
    if !adapter.is_powered().await? {
        eprintln!("Adapter is not on");
        return Ok(device_map)
    }
    if adapter.is_discovering().await? {
        eprintln!("Adapter is already discovering");
        return Ok(device_map)
    }

    // filter to just the mi band service
    let filter = DiscoveryFilter {
        uuids: HashSet::from([SERVICE_BAND_0]),
        transport: DiscoveryTransport::Le,
        duplicate_data: false,
        discoverable: false,
        ..Default::default()
    };
    
    adapter.set_discovery_filter(filter).await?;
    // start discovering
    let devices = adapter.discover_devices().await?;
    pin_mut!(devices);
    while let Some(event) = devices.next().await {
        match event {
            AdapterEvent::DeviceAdded(addr) => {
                if let Ok(device) = adapter.device(addr.clone()) {
                    // add it to the map
                    let name = device.name().await?.unwrap();
                    device_map.insert(addr, name);
                    break;
                }
            },
            AdapterEvent::DeviceRemoved(addr) => {
                device_map.remove(&addr);
            }
            _ => {}
        }
    }
    
    Ok(device_map)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let session = Session::new().await?;
    // get the adapter
    let adapter = session.default_adapter().await?;
    println!("{:?}", discover_mi_bands(&adapter).await?);

    Ok(())
}
