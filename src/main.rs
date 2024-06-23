use std::{collections::{HashMap, HashSet}, error::Error, time::Duration};

use bluer::{Adapter, AdapterEvent, Address, Device, DiscoveryFilter, DiscoveryTransport, Session, Uuid, id::{Characteristic, Service}};
use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use futures::pin_mut;
use tokio_stream::StreamExt;

macro_rules! uuid {
    ($var_name:ident,$uuid:expr) => {
        const $var_name: Uuid = match Uuid::try_parse($uuid) {
            Ok(u) => u,
            _ => panic!("bad uuid")
        };
    }
}

uuid!(SERVICE_BAND_0, "0000fee0-0000-1000-8000-00805f9b34fb");
uuid!(CHAR_BATTERY, "00000006-0000-3512-2118-0009af100700");
uuid!(CHAR_STEPS, "00000007-0000-3512-2118-0009af100700");

async fn discover_mi_bands(adapter: &Adapter, duration_ms: u64) -> bluer::Result<HashMap<Address, Device>> {
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
    let devices = adapter.discover_devices().await?.timeout(Duration::from_millis(duration_ms));
    pin_mut!(devices);
    while let Some(Ok(event)) = devices.next().await {
        match event {
            AdapterEvent::DeviceAdded(addr) => {
                if let Ok(device) = adapter.device(addr.clone()) {
                    // limit to devices that are actually present
                    // add it to the map
                    device_map.insert(addr, device);
                    
                    // temp helper:
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

// parse a time out of a 7 byte array
fn parse_time(value: &[u8]) -> Option<DateTime<Local>> {
    if value.len() < 7 { return None }
    
    let year = (value[0] as u16) | ((value[1] as u16) << 8);
    let month = value[2];
    let day = value[3];
    let hour = value[4];
    let minute = value[5];
    let second = value[6];

    let time = Local.with_ymd_and_hms(year.into(), month.into(), day.into(), hour.into(), minute.into(), second.into());
    match time {
        chrono::offset::LocalResult::Single(time) => Some(time),
        _ => None
    }
}

fn time_to_bytes(time: &DateTime<Local>) -> Vec<u8> {
    let year = time.year();
    vec![(year & 0xff) as u8, (year >> 8) as u8, time.month() as u8, time.day() as u8, time.hour() as u8, time.minute() as u8, time.second() as u8]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let session = Session::new().await?;
    // get the adapter
    let adapter = session.default_adapter().await?;
    let found = discover_mi_bands(&adapter, 50000).await?;
    let test_device = found.values().next().unwrap();
    println!("connecting to device {}", test_device.address());
    if !test_device.is_connected().await? {
        test_device.connect().await?;
    }
    println!("connected successfully");

    let services = test_device.services().await?;
    let mut service_band_0 = None;
    let mut service_device_info = None;
    for service in services {
        let uuid = service.uuid().await?;
        if uuid == SERVICE_BAND_0 {
            service_band_0 = Some(service);
        } else if uuid == Service::DeviceInformation.into() {
            service_device_info = Some(service);
        }
    }
    for characteristic in service_band_0.unwrap().characteristics().await? {
        let uuid = characteristic.uuid().await?;
        if uuid == CHAR_BATTERY {
            let value = characteristic.read().await?;
            let battery_level = value[1];
            let charging = value[2] != 0;

            let last_off = parse_time(&value[3..]).unwrap().to_string();
            let last_charge = parse_time(&value[11..]).unwrap().to_string();
            
            println!("Battery level: {battery_level}\nCharging: {charging}\nLast off: {last_off}\nLast charge: {last_charge}");
        } else if uuid == Characteristic::CurrentTime.into() {
            let value = characteristic.read().await?;
            //println!("{:?}", value);
            println!("time: {:?}", parse_time(&value));
            println!("syncing current time");
            let mut now = time_to_bytes(&Local::now());
            let mut rest: Vec<u8> = vec![Local::now().weekday().num_days_from_sunday() as u8, 0, 0, 0];
            now.append(&mut rest);
            //println!("{:?}", now);
            /*let req = CharacteristicWriteRequest {
                offset: 0,
                op_type: bluer::gatt::WriteOp::Request,
                prepare_authorize: false,
                ..Default::default()
            };*/
            // this doesn't work yet because we need to authenticate
            // characteristic.write_ext(&now, &req).await?;
        }/* this also requires auth
        else if uuid == CHAR_STEPS {
            let value = characteristic.read().await?;
            let steps = (value[1] as u16) & ((value[2] as u16) << 8);
            let meters = (value[5] as u16) & ((value[6] as u16) << 8);
            let calories = (value[9] as u16) & ((value[10] as u16) << 8);
            println!("Steps: {steps}, Meters: {meters}, Calories burned: {calories}");
    }*/
    }

    for characteristic in service_device_info.unwrap().characteristics().await? {
        let uuid = characteristic.uuid().await?;
        let value = characteristic.read().await?;
        if uuid == Characteristic::HardwareRevisionString.into() {
            println!("Hardware revision: {}", String::from_utf8(value).unwrap());
        } else if uuid == Characteristic::SoftwareRevisionString.into() {
            println!("Software revision: {}", String::from_utf8(value).unwrap());
        }
    }

    Ok(())
}
