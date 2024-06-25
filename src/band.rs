use std::{collections::HashMap, error::Error, fmt::Display, io, time::Duration};

use async_io::Timer;
use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use futures::{pin_mut, select, FutureExt, StreamExt};
use zbus::zvariant::OwnedObjectPath;

use crate::{bluez::{AdapterProxy, BluezSession, Device, DeviceEvent, DiscoveryFilter}, utils::encrypt_value};

const SERVICE_BAND_0: &'static str = "0000fee0-0000-1000-8000-00805f9b34fb";
const SERVICE_BAND_1: &'static str = "0000fee1-0000-1000-8000-00805f9b34fb";
const CHAR_BATTERY: &'static str = "00000006-0000-3512-2118-0009af100700";
const CHAR_STEPS: &'static str = "00000007-0000-3512-2118-0009af100700";
const CHAR_AUTH: &'static str = "00000009-0000-3512-2118-0009af100700";

struct BandChars {
    battery: OwnedObjectPath,
    steps: OwnedObjectPath,
    firm_rev: OwnedObjectPath,
    time: OwnedObjectPath,
    auth: OwnedObjectPath
}

#[derive(Debug)]
pub enum BandError {
    DBusError(zbus::Error),
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

impl From<zbus::Error> for BandError {
    fn from(value: zbus::Error) -> Self {
        Self::DBusError(value)
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

#[derive(Debug)]
pub struct CurrentActivity {
    steps: u16,
    calories: u16,
    meters: u16
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

/*// helper functions for getting all services/chars
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
}*/

impl MiBand {
    /*pub fn new(device: Device) -> Self {
        Self {
            device,
            authenticated: false,
            chars: None
        }
    }

    pub async fn is_connected(&self) -> bool {
        self.device.is_connected().await.unwrap_or(false)
    }

    /// connect to the band and fetch all GATT characteristics
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

    /// iterate through all the services and characteristics in order to find the ones we need
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

            // note: it's important that we start the notify session before writing
            let notify = auth.notify_io().await?;
            let write = auth.write_io().await?;

            // signal the band to start auth
            write.send(&[0x02, 0x00]).await?;
            loop {
                let value = notify.recv().await?;
                if value[0] == 0x10 && value.len() >= 3 {
                    match &value[1..3] {
                        &[0x01, 0x01] => {
                            // signal to start again
                            write.send(&[0x02, 0x00]).await?;
                        },
                        &[0x02, 0x01] => {
                            // the band has sent us a 16 byte value to encrypt
                            let value = &value[3..19];
                            if let Some(encrypted) = encrypt_value(&auth_key, value) {
                                // 0x03 0x00 <first 16 bytes of encrypted value>
                                let response = [&[0x03, 0x00], &encrypted[0..16]].concat();
                                write.send(&response).await?;
                            }
                        },
                        &[0x03, 0x01] => {
                            // success
                            self.authenticated = true;
                            return Ok(());
                        },
                        &[0x03, 0x08] => {
                            // invalid auth key
                            return Err(BandError::InvalidAuthKey);
                        },
                        
                        value => {
                            println!("unknown authentication response {value:?}");
                        }
                    }
                }
            }
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

    /// set the time on the band to a specific value
    pub async fn set_band_time(&self, new_time: DateTime<Local>) -> Result<()> {
        if !self.authenticated { return Err(BandError::RequiresAuth) }
        
        if let Some(BandChars { time, .. }) = &self.chars {
            let year = new_time.year();
            let day_of_week = new_time.weekday().num_days_from_sunday() as u8;
            // year (two bytes), month, day, hour, minute, second, day of week, 3 zeros? (could be timezone)
            let value = vec![(year & 0xff) as u8, (year >> 8) as u8, new_time.month() as u8, new_time.day() as u8, new_time.hour() as u8, new_time.minute() as u8, new_time.second() as u8, day_of_week, 0, 0, 0];
            let write_req = CharacteristicWriteRequest {
                op_type: WriteOp::Request,
                prepare_authorize: true,
                ..Default::default()
            };
            time.write_ext(&value, &write_req).await?;
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    /// get the current step count, meters walked, and calories burned
    pub async fn get_current_activity(&self) -> Result<CurrentActivity> {
        if let Some(BandChars { steps, .. }) = &self.chars {
            let value = steps.read().await?;
            let steps = (value[1] as u16) | ((value[2] as u16) << 8);
            let meters = (value[5] as u16) | ((value[6] as u16) << 8);
            let calories = (value[9] as u16) | ((value[10] as u16) << 8);
            Ok(CurrentActivity {
                steps, meters, calories
            })
        } else { Err(BandError::NotInitialized) }
    }

    // firmware revision (software revision string)
    pub async fn get_firmware_revision(&self) -> Result<String> {
        if let Some(BandChars { firm_rev, .. }) = &self.chars {
            let value = firm_rev.read().await?;
            String::from_utf8(value).map_err(|_e| BandError::Utf8Error)
        } else { Err(BandError::NotInitialized) }
    }*/

    /// discover valid mi bands in the area
    pub async fn discover<'a>(session: BluezSession<'a>, timeout: Duration)  -> Result<HashMap<OwnedObjectPath, String>> {

        let existing_devices = session.get_devices().await?;
        let mut device_map: HashMap<OwnedObjectPath, String> = existing_devices.into_iter()
            .filter_map(|device| {
                if device.services.contains(SERVICE_BAND_0) {
                    // this is a mi band
                    Some((device.path, device.address))
                } else { None }
            })
            .collect();

        // filter to just the mi band service
        let filter = DiscoveryFilter {
            uuids: vec![SERVICE_BAND_0.into()],
            transport: "le".into(),
            duplicate_data: false
        };

        // start discovery
        session.adapter.set_discovery_filter(filter).await?;
        session.adapter.start_discovery().await?;
        let stream = session.stream_device_events().await?.fuse();
        pin_mut!(stream);
        let mut timeout = FutureExt::fuse(Timer::after(timeout));
        loop {
            select! {
                event = stream.next() => {
                    match event {
                        Some(DeviceEvent::DeviceAdded(Device { address, path, services })) => {
                             // add each new device to the device map as long as it contains our service
                            // we need to check for the service here because the DiscoveryFilter isn't reliable
                            if services.contains(SERVICE_BAND_0) {
                                device_map.insert(path, address);
                            }
                        },
                        Some(DeviceEvent::DeviceRemoved(path)) => {
                            device_map.remove(&path);
                        },
                        _ => {}
                    }
                },
                _ = timeout => { break; }
            }
        }
        session.adapter.stop_discovery().await?;
        
        Ok(device_map)
    }
}
