use std::{error::Error, fmt::Display, io};

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use futures::{AsyncReadExt, AsyncWriteExt,  Stream, StreamExt, stream::select};
use zbus::zvariant::OwnedObjectPath;

use crate::{bluez::{BluezSession, DeviceProxy, DiscoveredDevice, DiscoveredDeviceEvent, DiscoveryFilter, GattCharacteristicProxy, WriteOptions}, store, utils::encrypt_value};

const SERVICE_BAND_0: &'static str = "0000fee0-0000-1000-8000-00805f9b34fb";
const SERVICE_BAND_1: &'static str = "0000fee1-0000-1000-8000-00805f9b34fb";
const SERVICE_DEVICE_INFO: &'static str = "0000180a-0000-1000-8000-00805f9b34fb";
const CHAR_BATTERY: &'static str = "00000006-0000-3512-2118-0009af100700";
const CHAR_STEPS: &'static str = "00000007-0000-3512-2118-0009af100700";
const CHAR_AUTH: &'static str = "00000009-0000-3512-2118-0009af100700";
const CHAR_SOFT_REV: &'static str = "00002a28-0000-1000-8000-00805f9b34fb";
const CHAR_TIME: &'static str = "00002a2b-0000-1000-8000-00805f9b34fb";

#[derive(Debug)]
struct BandChars<'a> {
    battery: GattCharacteristicProxy<'a>,
    steps: GattCharacteristicProxy<'a>,
    firm_rev: GattCharacteristicProxy<'a>,
    time: GattCharacteristicProxy<'a>,
    auth: GattCharacteristicProxy<'a>
}

#[derive(Debug)]
pub enum BandError {
    DBusError(zbus::Error),
    IoError(io::Error),
    StoreError(store::Error),
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

impl From<store::Error> for BandError {
    fn from(value: store::Error) -> Self {
        Self::StoreError(value)
    }
}

impl Display for BandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for BandError {}

pub type Result<T> = std::result::Result<T, BandError>;

#[derive(Debug)]
pub struct MiBand<'a> {
    session: BluezSession<'a>,
    device: DeviceProxy<'a>,
    pub authenticated: bool,
    chars: Option<BandChars<'a>>,
    pub address: String
}

#[derive(Debug)]
pub struct BatteryStatus {
    pub battery_level: u8,
    pub last_off: DateTime<Local>,
    pub last_charge: DateTime<Local>,
    pub charging: bool
}

#[derive(Debug)]
pub struct CurrentActivity {
    steps: u16,
    calories: u16,
    meters: u16
}

#[derive(Debug)]
pub enum BandChangeEvent {
    RSSI(Option<i16>),
    Connected(bool)
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

impl<'a> MiBand<'a> {
    pub async fn from_discovered_device<'b>(session: BluezSession<'a>, device: DiscoveredDevice) -> Result<Self> {
        let device_proxy = session.proxy_from_discovered_device(device.path).await?;
        Ok(Self {
            device: device_proxy,
            session,
            authenticated: false,
            chars: None,
            address: device.address
        })
    }

    pub async fn initialize<'b>(&'b mut self) -> Result<()> {
        // first connect if needed
        let was_connected = self.is_connected().await;
        if !was_connected {
            self.device.connect().await?;
        }

        // if we weren't connected of if we don't have the chars, fetch them
        if !was_connected || self.chars.is_none() {
            let chars = self.fetch_chars().await?;
            self.chars = Some(chars);
        }

        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        self.device.connected().await.unwrap_or(false)
    }

    /// iterate through all the services and characteristics in order to find the ones we need
    /// Note: device must be connected here
    async fn fetch_chars<'b>(&self) -> Result<BandChars<'b>> {
        let services_resolved = self.device.services_resolved().await.unwrap_or(false);

        if !services_resolved {
            // wait for services to resolve
            let mut services_resolved_stream = self.device.receive_services_resolved_changed().await;
            while let Some(value) = services_resolved_stream.next().await {
                if let Ok(true) = value.get().await { break; }
            }
        };
        
        // get the services
        let mut services = self.session.get_device_characteristics(self.device.path()).await?;
        match (
            services.remove(SERVICE_BAND_0),
            services.remove(SERVICE_BAND_1),
            services.remove(SERVICE_DEVICE_INFO)
        ) {
            (Some(mut band_0), Some(mut band_1), Some(mut device_info)) => {
                // get the characteristics from their respective services
                match (
                    band_0.remove(CHAR_BATTERY),
                    band_0.remove(CHAR_STEPS),
                    band_0.remove(CHAR_TIME),
                    device_info.remove(CHAR_SOFT_REV),
                    band_1.remove(CHAR_AUTH)
                ) {
                    (Some(battery), Some(steps), Some(time), Some(firm_rev), Some(auth)) => {
                        let chars = BandChars {
                            battery, steps, time, firm_rev, auth
                        };

                        return Ok(chars);
                    },
                    _ => {}
                }
            },
            _ => {}
        }

        return Err(BandError::MissingServicesOrChars);
    }

    pub async fn disconnect(&mut self) -> Result<()> {
        self.device.disconnect().await?;
        self.authenticated = false;
        Ok(())
    }

    /// Authenticate with the band
    pub async fn authenticate(&mut self, auth_key: &[u8]) -> Result<()> {
        if let Some(BandChars { auth, ..}) = &self.chars {

            // note: it's important that we start the notify session before writing
            let (mut notify, notify_mtu) = auth.acquire_notify_stream().await?;
            let (mut write, _) = auth.acquire_write_stream().await?;

            // signal the band to start auth
            write.write(&[0x02, 0x00]).await?;
            let mut buf = vec![0; notify_mtu as usize];
            loop {
                let len = notify.read(&mut buf).await?;
                if len >= 3 && buf[0] == 0x10 {
                    match &buf[1..3] {
                        &[0x01, 0x01] => {
                            // signal to start again
                            write.write(&[0x02, 0x00]).await?;
                        },
                        &[0x02, 0x01] => {
                            // the band has sent us a 16 byte value to encrypt
                            let value = &buf[3..19];
                            if let Some(encrypted) = encrypt_value(&auth_key, value) {
                                // 0x03 0x00 <first 16 bytes of encrypted value>
                                let response = [&[0x03, 0x00], &encrypted[0..16]].concat();
                                write.write(&response).await?;
                            }
                        },
                        &[0x03, 0x01] => {
                            // success
                            self.authenticated = true;
                            return Ok(());
                        },
                        &[0x03, 0x08] => {
                            self.authenticated = false;
                            // invalid auth key
                            return Err(BandError::InvalidAuthKey);
                        },
                        
                        buf => {
                            println!("unknown authentication response {buf:?}");
                        }
                    }
                }
            }
        } else { Err(BandError::NotInitialized) }
    }

    /// get the battery level and status
    pub async fn get_battery(&self) -> Result<BatteryStatus> {
        if let Some(BandChars { battery, .. }) = &self.chars {
            let value = battery.read_value_default().await?;
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
            let value = time.read_value_default().await?;
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
            let write_options = WriteOptions {
                write_type: "request".into(),
                prepare_authorize: true,
                offset: 0
            };
            time.write_value(&value, &write_options).await?;
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    /// get the current step count, meters walked, and calories burned
    pub async fn get_current_activity(&self) -> Result<CurrentActivity> {
        if let Some(BandChars { steps, .. }) = &self.chars {
            let value = steps.read_value_default().await?;
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
            let value = firm_rev.read_value_default().await?;
            String::from_utf8(value).map_err(|_e| BandError::Utf8Error)
        } else { Err(BandError::NotInitialized) }
    }

    pub async fn get_known_bands<'b>(session: &'b BluezSession<'b>) -> Result<Vec<DiscoveredDevice>> {
        let existing_devices = session.get_devices().await?;
        Ok(existing_devices.into_iter().filter(|device| device.services.contains(SERVICE_BAND_0)).collect())
    }

    pub async fn start_filtered_discovery<'b>(session: BluezSession<'b>) -> Result<()> {
        // filter to just the mi band service
        let filter = DiscoveryFilter {
            uuids: vec![SERVICE_BAND_0.into()],
            transport: "le".into(),
            duplicate_data: false
        };

        // start discovery
        session.adapter.set_discovery_filter(filter).await?;
        session.adapter.start_discovery().await?;
        Ok(())
    }

    /// discover valid mi bands in the area
    pub async fn stream_known_bands<'b>(session: &'b BluezSession<'b>)  -> Result<impl Stream<Item = DiscoveredDeviceEvent> + 'b> {
        let stream = session.stream_device_events().await?;
        Ok(stream.filter_map(move |item| async {
            // make sure added devices are mi bands
            if let DiscoveredDeviceEvent::DeviceAdded(device) = &item {
                if !device.services.contains(SERVICE_BAND_0) { return None };
            }
            Some(item)
        }))
    }

    pub async fn stream_band_events<'b, 'c>(session: &'b BluezSession<'b>, path: OwnedObjectPath) -> Result<impl Stream<Item = (OwnedObjectPath, BandChangeEvent)>> {
        let proxy = session.proxy_from_discovered_device(path.clone()).await?;
        let path2 = path.clone();
        let rssi = proxy.receive_rssi_changed().await
            .then(move |v| {
                let path = path.clone();
                async move {
                    // rssi may not exist
                    (path.clone(), BandChangeEvent::RSSI(v.get().await.ok()))
                }
            });
        // stream changes for the connected event too
        let connected = proxy.receive_connected_changed().await
            .then(move |v| {
                let path = path2.clone();
                async move {
                    (path.clone(), BandChangeEvent::Connected(v.get().await.unwrap_or(false)))
                }
            });


        Ok(select(rssi, connected))
    }
}
