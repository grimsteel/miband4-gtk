use std::{collections::{HashMap, HashSet}, error::Error, fmt::Display, io, time::Duration};

use bluer::{gatt::remote::{Service, Characteristic}, id::{Characteristic as CharId, Service as ServiceId}, Adapter, AdapterEvent, Address, Device, DiscoveryFilter, DiscoveryTransport, Uuid};
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
uuid!(SERVICE_BAND_1, "0000fee1-0000-1000-8000-00805f9b34fb");
uuid!(CHAR_BATTERY, "00000006-0000-3512-2118-0009af100700");
uuid!(CHAR_STEPS, "00000007-0000-3512-2118-0009af100700");
uuid!(CHAR_AUTH, "00000009-0000-3512-2118-0009af100700");

struct BandChars {
    battery: Characteristic,
    steps: Characteristic,
    firm_rev: Characteristic,
    time: Characteristic,
    auth: Characteristic
}

#[derive(Debug)]
pub enum BandError {
    BluerError(bluer::Error),
    IoError(io::Error),
    MissingServicesOrChars,
    NotInitialized,
    InvalidTime,
    Utf8Error,
    RequiresAuth,
    InvalidAuthKey,
    Failed,
    UnknownError
}

impl From<bluer::Error> for BandError {
    fn from(value: bluer::Error) -> Self {
        Self::BluerError(value)
    }
}

impl From<io::Error> for BandError {
    fn from(value: io::Error) -> Self {
        Self::IoError(value)
    }
}

impl Display for BandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for BandError {}

type Result<T> = std::result::Result<T, BandError>;

pub struct MiBand {
    device: Device,
    pub authenticated: bool,
    chars: Option<BandChars>
}

#[derive(Debug)]
pub struct BatteryStatus {
    battery_level: u8,
    last_off: DateTime<Local>,
    last_charge: DateTime<Local>,
    charging: bool
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

// helper functions for getting all services/chars
async fn get_all_services(device: &Device) -> Result<HashMap<Uuid, Service>> {
    let mut map = HashMap::new();
    for service in device.services().await? {
        let uuid = service.uuid().await?;
        map.insert(uuid, service);
    }
    Ok(map)
}

async fn get_all_chars(service: &Service) -> Result<HashMap<Uuid, Characteristic>> {
    let mut map = HashMap::new();
    for characteristic in service.characteristics().await? {
        let uuid = characteristic.uuid().await?;
        map.insert(uuid, characteristic);
    }
    Ok(map)
}

impl MiBand {
    pub fn new(device: Device) -> Self {
        Self {
            device,
            authenticated: false,
            chars: None
        }
    }

    pub async fn is_connected(&self) -> bool {
        self.device.is_connected().await.unwrap_or(false)
    }

    pub async fn initialize(&mut self) -> Result<()> {
        // first connect if needed
        let was_connected = self.is_connected().await;
        if !was_connected {
            self.device.connect().await?;
        }

        // if we weren't connected of if we don't have the chars, fetch them
        if !was_connected || self.chars.is_none() {
            self.fetch_chars().await?;
        }

        Ok(())
    }

    /// Note: device must be connected here
    async fn fetch_chars(&mut self) -> Result<()> {
        // get the services
        let services = get_all_services(&self.device).await?;

        match (services.get(&SERVICE_BAND_0), services.get(&SERVICE_BAND_1), services.get(&ServiceId::DeviceInformation.into())) {
            (Some(band_0), Some(band_1), Some(device_info)) => {
                let mut band_0 = get_all_chars(band_0).await?;
                let mut band_1 = get_all_chars(band_1).await?;
                let mut device_info = get_all_chars(device_info).await?;
                // get the characteristics from their respective services
                match (band_0.remove(&CHAR_BATTERY), band_0.remove(&CHAR_STEPS), band_0.remove(&CharId::CurrentTime.into()), device_info.remove(&CharId::SoftwareRevisionString.into()), band_1.remove(&CHAR_AUTH)) {
                    (Some(battery), Some(steps), Some(time), Some(firm_rev), Some(auth)) => {
                        let chars = BandChars {
                            battery, steps, time, firm_rev, auth
                        };

                        self.chars = Some(chars);

                        return Ok(());
                    },
                    _ => {}
                }
            },
            _ => {}
        }

        return Err(BandError::MissingServicesOrChars);
    }

    /// Authenticate with the band
    pub async fn authenticate(&mut self, auth_key: &[u8]) -> Result<()> {
        if let Some(BandChars { auth, ..}) = &self.chars {
            let write_io = auth.write_io().await?;

            // start authentication
            println!("starting auth");
            write_io.send(&[0x02, 0x00]).await?;
            println!("starting notify");
            let notify = auth.notify_io().await?;
//            pin_mut!(notify);
            loop {
                match notify.recv().await {
                    Ok(value) => {
                        println!("{:?}", value);
                    },
                    _ => break
                }
                break;
            }
            println!("done");
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    /// get the battery level and status
    pub async fn get_battery(&self) -> Result<BatteryStatus> {
        if let Some(BandChars { battery, .. }) = &self.chars {
            let value = battery.read().await?;
            let battery_level = value[1];
            let charging = value[2] != 0;

            let last_off = parse_time(&value[3..]).ok_or(BandError::InvalidTime)?;
            let last_charge = parse_time(&value[11..]).ok_or(BandError::InvalidTime)?;

            Ok(BatteryStatus {
                battery_level,
                charging,
                last_off,
                last_charge
            })
        } else { Err(BandError::NotInitialized) }
    }

    /// get the current time on the band
    pub async fn get_band_time(&self) -> Result<DateTime<Local>> {
        if let Some(BandChars { time, .. }) = &self.chars {
            let value = time.read().await?;
            parse_time(&value).ok_or(BandError::InvalidTime)
        } else { Err(BandError::NotInitialized) }
    }

    pub async fn set_band_time(&self, new_time: DateTime<Local>) -> Result<()> {
        if let Some(BandChars { time, .. }) = &self.chars {
            let year = new_time.year();
            let day_of_week = new_time.weekday().num_days_from_sunday() as u8;
            // year (two bytes), month, day, hour, minute, second, day of week, 3 zeros? (could be timezone)
            let value = vec![(year & 0xff) as u8, (year >> 8) as u8, new_time.month() as u8, new_time.day() as u8, new_time.hour() as u8, new_time.minute() as u8, new_time.second() as u8, day_of_week, 0, 0, 0];
            time.write(&value).await?;
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    pub async fn get_firmware_revision(&self) -> Result<String> {
        if let Some(BandChars { firm_rev, .. }) = &self.chars {
            let value = firm_rev.read().await?;
            String::from_utf8(value).map_err(|_e| BandError::Utf8Error)
        } else { Err(BandError::NotInitialized) }
    }

    /// discover valid mi bands in the area
    pub async fn discover(adapter: &Adapter, timeout: Duration)  -> Result<HashMap<Address, Device>> {
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
        let devices = adapter.discover_devices().await?.timeout(timeout);
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
}
