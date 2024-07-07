use std::{error::Error, fmt::Display, io, pin::Pin, task::{Context, Poll}};

use async_net::unix::UnixStream;
use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use futures::{stream::select, AsyncRead, AsyncReadExt, AsyncWriteExt, Stream, StreamExt};
use zbus::zvariant::{ObjectPath, OwnedObjectPath};

use crate::{bluez::{BluezSession, DeviceProxy, DiscoveredDevice, DiscoveredDeviceEvent, DiscoveryFilter, GattCharacteristicProxy}, mpris::{MediaInfo, MediaState}, store::{self, ActivityGoal, BandLock}, utils::encrypt_value};

const SERVICE_BAND_0: &'static str = "0000fee0-0000-1000-8000-00805f9b34fb";
const SERVICE_BAND_1: &'static str = "0000fee1-0000-1000-8000-00805f9b34fb";
const SERVICE_DEVICE_INFO: &'static str = "0000180a-0000-1000-8000-00805f9b34fb";
const SERVICE_NOTIFICATION: &'static str = "00001811-0000-1000-8000-00805f9b34fb";
const CHAR_BATTERY: &'static str = "00000006-0000-3512-2118-0009af100700";
const CHAR_STEPS: &'static str = "00000007-0000-3512-2118-0009af100700";
const CHAR_AUTH: &'static str = "00000009-0000-3512-2118-0009af100700";
const CHAR_SOFT_REV: &'static str = "00002a28-0000-1000-8000-00805f9b34fb";
const CHAR_TIME: &'static str = "00002a2b-0000-1000-8000-00805f9b34fb";
const CHAR_CONFIG: &'static str = "00000003-0000-3512-2118-0009af100700";
const CHAR_SETTINGS: &'static str = "00000008-0000-3512-2118-0009af100700";
const CHAR_ALERT: &'static str = "00002a46-0000-1000-8000-00805f9b34fb";
const CHAR_CHUNKED_TRANSFER: &'static str = "00000020-0000-3512-2118-0009af100700";
const CHAR_MUSIC_NOTIFICATIONS: &'static str = "00000010-0000-3512-2118-0009af100700";

#[derive(Debug)]
struct BandChars<'a> {
    battery: GattCharacteristicProxy<'a>,
    steps: GattCharacteristicProxy<'a>,
    firm_rev: GattCharacteristicProxy<'a>,
    time: GattCharacteristicProxy<'a>,
    auth: GattCharacteristicProxy<'a>,
    config: GattCharacteristicProxy<'a>,
    settings: GattCharacteristicProxy<'a>,
    alert: GattCharacteristicProxy<'a>,
    chunked_transfer: GattCharacteristicProxy<'a>,
    music_notifs: GattCharacteristicProxy<'a>
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
    InvalidLockPin,
    //Failed,
    //UnknownError
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

// Human-readable BandErrors
impl Display for BandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DBusError(err) => write!(f, "D-Bus error: {}", err),
            Self::IoError(err) => write!(f, "I/O error: {}", err),
            Self::StoreError(err) => write!(f, "Store error: {}", err),
            Self::MissingServicesOrChars => write!(f, "Device is missing required BLE services or characteristics"),
            Self::NotInitialized => write!(f, "Device connection is not initialized"),
            Self::InvalidTime => write!(f, "Device sent an invalid time"),
            Self::Utf8Error => write!(f, "Device sent invalid UTF-8 text"),
            Self::RequiresAuth => write!(f, "The operation requires authentication"),
            Self::InvalidAuthKey => write!(f, "Invalid auth key"),
            Self::InvalidLockPin => write!(f, "Invalid band lock PIN (must be 4 digits from 1-4)"),
            //Self::Failed => write!(f, "The operation failed"),
            //Self::UnknownError => write!(f, "An unknown error occurred")
        }
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
    //pub last_off: DateTime<Local>,
    pub last_charge: DateTime<Local>,
    pub charging: bool
}

#[derive(Debug)]
pub struct CurrentActivity {
    pub steps: u16,
    pub calories: u16,
    pub meters: u16
}

#[derive(Debug)]
pub enum BandChangeEvent {
    RSSI(Option<i16>),
    Connected(bool)
}

#[derive(Copy, Clone)]
pub enum AlertType {
    Mail = 0x01,
    Call = 0x03,
    MissedCall = 0x04,
    Message = 0x05
}

pub struct Alert<'a> {
    pub alert_type: AlertType,
    pub title: &'a str,
    pub message: &'a str
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum MusicEvent {
    // open/close the music screen on the band
    Open,
    Close,

    PlayPause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown
}


/// A `Stream` implementation for music events from a band
#[derive(Debug)]
pub struct MusicEventListener {
    notify_stream: UnixStream,
    mtu: usize
}

impl Stream for MusicEventListener {
    type Item = MusicEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut buf = vec![0; self.mtu];
        let result = Pin::new(&mut self.get_mut().notify_stream).poll_read(cx, &mut buf);
        let result = result.map(move |value| -> io::Result<Option<MusicEvent>> {
            // when this function returns an Err, that means the stream must end
            let size = value?;
            let buf = &buf[..size];
            // Ok(None) means we don't recognize the data it gave us
            if size < 2 { return Ok(None) }
            Ok(match buf[1] {
                0xe0 => Some(MusicEvent::Open),
                0xe1 => Some(MusicEvent::Close),
                d => {
                    println!("{d}");
                    None
                }
            })
        });

        match result {
            Poll::Ready(Ok(Some(v))) => Poll::Ready(Some(v)),
            // fatal - stream must end
            Poll::Ready(Err(_)) => Poll::Ready(None),
            // the band sent data we don't recognize, but it's not fatal
            // we just don't have data to send
            Poll::Ready(Ok(None)) => Poll::Pending,
            Poll::Pending => Poll::Pending
        }
    }
}

/// parse a time out of a 7 byte array
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

    pub fn path<'b>(&'b self) -> &'b ObjectPath {
        self.device.path()
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
            services.remove(SERVICE_DEVICE_INFO),
            services.remove(SERVICE_NOTIFICATION)
        ) {
            
            (Some(mut band_0), Some(mut band_1), Some(mut device_info), Some(mut notification)) => {
                // get the characteristics from their respective services
                match (
                    band_0.remove(CHAR_BATTERY),
                    band_0.remove(CHAR_STEPS),
                    band_0.remove(CHAR_TIME),
                    band_0.remove(CHAR_CONFIG),
                    band_0.remove(CHAR_SETTINGS),
                    band_0.remove(CHAR_CHUNKED_TRANSFER),
                    band_0.remove(CHAR_MUSIC_NOTIFICATIONS),
                    device_info.remove(CHAR_SOFT_REV),
                    band_1.remove(CHAR_AUTH),
                    notification.remove(CHAR_ALERT)
                ) {
                    (
                        Some(battery),
                        Some(steps),
                        Some(time),
                        Some(config),
                        Some(settings),
                        Some(chunked_transfer),
                        Some(music_notifs),
                        Some(firm_rev),
                        Some(auth),
                        Some(alert)
                    ) => {
                        let chars = BandChars {
                            battery, steps, time, config, firm_rev, auth, settings, alert, chunked_transfer, music_notifs
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

    /// chunked data transfer for longer payloads
    async fn write_chunked(&self, message_type: u8, payload: &[u8]) -> Result<()> {
        const CHUNK_LENGTH: usize = 17;
        if let Some(BandChars { chunked_transfer, .. }) = &self.chars {
            let chunks = payload.chunks(CHUNK_LENGTH).enumerate();
            let num_chunks = chunks.len();
            let processed_chunks: Vec<_> = chunks.map(|(i, chunk)| {
                let flag = match (i == 0, i == num_chunks - 1) {
                    // first and last chunk
                    (true, true) => 0x40 | 0x80,
                    // first chunk
                    (true, false) => 0,
                    // last chunk
                    (false, true) => 0x80,
                    // middle chunk
                    (false, false) => 0x40
                } | message_type;
                // 0x00 <flag> <num chunks> <data...>
                [&[0x00, flag, (i & 0xff) as u8], chunk].concat()
            }).collect();

            // write all of the chunks
            for chunk in processed_chunks {
                chunked_transfer.write_value_command(&chunk).await?;
            }
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    /// get the battery level and status
    pub async fn get_battery(&self) -> Result<BatteryStatus> {
        if let Some(BandChars { battery, .. }) = &self.chars {
            let value = battery.read_value_default().await?;
            let battery_level = value[1];
            let charging = value[2] != 0;

            //let last_off = parse_time(&value[3..]).ok_or(BandError::InvalidTime)?;
            let last_charge = parse_time(&value[11..]).ok_or(BandError::InvalidTime)?;

            Ok(BatteryStatus {
                battery_level,
                charging,
                //last_off,
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
            time.write_value_request(&value).await?;
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    /// get the current step count, meters walked, and calories burned
    pub async fn get_current_activity(&self) -> Result<CurrentActivity> {
        if !self.authenticated { return Err(BandError::RequiresAuth) }
        
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

    /// set the daily goal notification state + step count
    pub async fn set_activity_goal(&self, goal: &ActivityGoal) -> Result<()> {
        if !self.authenticated { return Err(BandError::RequiresAuth) }
        
        if let Some(BandChars { config, settings, .. }) = &self.chars {
            // enable/disable notifications
            let notifs_enabled_byte = if goal.notifications { 0x01 } else { 0x00 };
            config.write_value_command(&vec![0x06, 0x06, 0x00, notifs_enabled_byte]).await?;

            // set the actual goal
            let goal_payload = vec![0x10, 0x00, 0x00, (goal.steps & 0xff) as u8, (goal.steps >> 8) as u8, 0x00, 0x00];
            settings.write_value_request(&goal_payload).await?;
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    /// firmware revision (software revision string)
    pub async fn get_firmware_revision(&self) -> Result<String> {
        if let Some(BandChars { firm_rev, .. }) = &self.chars {
            let value = firm_rev.read_value_default().await?;
            String::from_utf8(value).map_err(|_e| BandError::Utf8Error)
        } else { Err(BandError::NotInitialized) }
    }

    /// show a notification on the band
    pub async fn send_alert(&self, alert_data: &Alert<'_>) -> Result<()> {
        if let Some(BandChars { alert, .. }) = &self.chars {
            let type_byte = alert_data.alert_type as u8;
            let data = [
                &[type_byte, 0x01],
                alert_data.title.as_bytes(),
                &[0x00],
                alert_data.message.as_bytes(),
                &[0x00]
            ].concat();
            alert.write_value_request(&data).await?;
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    pub async fn set_band_lock(&self, lock: &BandLock) -> Result<()> {
        if let Some(BandChars { config, .. }) = &self.chars {
            // make sure all digits are between 1-4
            if lock.pin.len() != 4 || !lock.pin.chars().all(|i| i >= '1' && i <= '4') { return Err(BandError::InvalidLockPin); }
            let data = [
                &[0x06, 0x21, 0x00, if lock.enabled { 0x01 } else { 0x00 }],
                &lock.pin.bytes().collect::<Vec<u8>>()[..],
                &[0x00]
            ].concat();
            config.write_value_command(&data).await?;
            Ok(())
        } else { Err(BandError::NotInitialized) }
    }

    pub async fn set_media_info(&self, media: &Option<MediaInfo>) -> Result<()> {
        if let Some(media) = media {
            let pos = media.position.unwrap_or_default();
            let all_fields = [
                // always include the position (even if it's just [0x00, 0x00])
                (0x00u8, Some(vec![(pos & 0xff) as u8, (pos > 8) as u8])),
                // track + null term
                (0x08u8, media.track.as_ref().map(|b| [b.as_bytes(), &[0x00]].concat())),
                // big endian duration + volume
                (0x10u8, media.duration.map(|d| vec![(d & 0xff) as u8, (d > 8) as u8])),
                (0x40u8, media.volume.map(|d| vec![(d & 0xff) as u8, (d > 8) as u8]))
            ];
            let (flags, bufs): (Vec<u8>, Vec<Vec<u8>>) = all_fields.into_iter()
                .filter_map(|(flag, buf)| {
                    // basically filter out the `None`s
                    Some((flag, buf?))
                })
                .unzip();

            // OR all of the flags together with 0x01
            let flag = flags.into_iter().fold(0x01, |acc, f| acc | f);
            let buf = bufs.concat();

            let buf = [
                &[flag, if media.state == MediaState::Playing { 1 } else { 0 }, 0x00],
                &buf[..]
            ].concat();

            self.write_chunked(0x03, &buf).await
        } else {
            self.write_chunked(0x03, &vec![0x00; 5]).await
        }
    }

    /// listen for the media button presses
    pub async fn stream_media_button_events(&self) -> Result<MusicEventListener> {
        if let Some(BandChars { music_notifs, .. }) = &self.chars {
            let (notify_stream, mtu) = music_notifs.acquire_notify_stream().await?;
            Ok(MusicEventListener { notify_stream, mtu: mtu as usize })
        } else { Err(BandError::NotInitialized) }
    }


    // ===== STATIC STRUCT METHODS ===== //
    
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
