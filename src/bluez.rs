use std::{collections::{HashMap, HashSet}, os::fd::OwnedFd};

use async_net::unix::UnixStream;
use zbus::{fdo::{InterfacesAdded, ObjectManagerProxy}, names::OwnedInterfaceName, proxy, zvariant::{DeserializeDict, ObjectPath, OwnedFd as ZOwnedFd, OwnedObjectPath, OwnedValue, SerializeDict, Type}, Connection};

use futures::stream::select;

use futures_util::StreamExt;

#[derive(DeserializeDict, SerializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct DiscoveryFilter {
    #[zvariant(rename = "UUIDs")]
    pub uuids: Vec<String>,
    #[zvariant(rename = "DuplicateData")]
    pub duplicate_data: bool,
    #[zvariant(rename = "Transport")]
    pub transport: String
}

#[derive(DeserializeDict, SerializeDict, Type, Default)]
#[zvariant(signature = "dict")]
pub struct ReadOptions {
    pub offset: u16
}

#[derive(DeserializeDict, SerializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct WriteOptions {
    pub offset: u16,
    /// either "command" (w/o response), "request" (w/ response), or "reliable"
    #[zvariant(rename = "type")]
    pub write_type: String,
    pub prepare_authorize: bool
}

#[derive(DeserializeDict, SerializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct BlankOptions {}

// #region Bluez interfaces

#[proxy(default_service = "org.bluez", default_path = "/org/bluez/hci0", interface = "org.bluez.Adapter1", gen_blocking = false)]
trait Adapter {
    fn get_discovery_filters(&self) -> zbus::Result<Vec<String>>;
    fn set_discovery_filter(&self, filter: DiscoveryFilter) -> zbus::Result<()>;
    fn start_discovery(&self) -> zbus::Result<()>;
    fn stop_discovery(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn powered(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn discovering(&self) -> zbus::Result<bool>;
}

#[proxy(default_service = "org.bluez", interface = "org.bluez.Device1", gen_blocking = false)]
trait Device {
    fn connect(&self) -> zbus::Result<()>;
    fn disconnect(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn address(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn connected(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn services_resolved(&self) -> zbus::Result<bool>;
    #[zbus(property, name="RSSI")]
    fn rssi(&self) -> zbus::Result<i16>;
}

impl<'a> DeviceProxy<'a> {
    pub fn path<'b>(&'b self) -> &'b ObjectPath { self.0.path() }
}

#[proxy(default_service = "org.bluez", interface = "org.bluez.GattService1", gen_blocking = false)]
trait GattService {
    #[zbus(property, name = "UUID")]
    fn uuid(&self) -> zbus::Result<String>;
}

#[proxy(default_service = "org.bluez", interface = "org.bluez.GattCharacteristic1", gen_blocking = false)]
trait GattCharacteristic {
    fn read_value(&self, options: &ReadOptions) -> zbus::Result<Vec<u8>>;
    fn write_value(&self, value: &[u8], options: &WriteOptions) -> zbus::Result<()>;

    fn acquire_write(&self, options: &BlankOptions) -> zbus::Result<(ZOwnedFd, u16)>;
    fn acquire_notify(&self, options: &BlankOptions) -> zbus::Result<(ZOwnedFd, u16)>;
    
    #[zbus(property, name = "UUID")]
    fn uuid(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn service(&self) -> zbus::Result<ObjectPath>;
}

impl<'a> GattCharacteristicProxy<'a> {
    pub async fn read_value_default(&self) -> zbus::Result<Vec<u8>> {
        self.read_value(&ReadOptions::default()).await
    }

    pub async fn acquire_write_stream(&self) -> zbus::Result<(UnixStream, u16)> {
        let (fd, mtu) = self.acquire_write(&BlankOptions {}).await?;
        // convert into std ownedfd
        let fd: OwnedFd = fd.into();
        // async unix stream
        let stream = UnixStream::try_from(fd)?;

        Ok((stream, mtu))
    }

    pub async fn acquire_notify_stream(&self) -> zbus::Result<(UnixStream, u16)> {
        let (fd, mtu) = self.acquire_notify(&BlankOptions {}).await?;
        let fd: OwnedFd = fd.into();
        let stream = UnixStream::try_from(fd)?;

        Ok((stream, mtu))
    }
}

// #endregion


// Map of service id to map of char id to proxy
pub type DeviceServiceChars<'a> = HashMap<String, HashMap<String, GattCharacteristicProxy<'a>>>;

#[derive(Debug)]
pub enum DiscoveredDeviceEvent {
    DeviceAdded(DiscoveredDevice),
    DeviceRemoved(OwnedObjectPath)
}

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub path: OwnedObjectPath,
    pub address: String,
    pub services: HashSet<String>,
    pub rssi: Option<i16>,
    pub connected: bool
}


#[derive(Debug, Clone)]
pub struct BluezSession<'a> {
    connection: Connection,
    pub adapter: AdapterProxy<'a>,
    object_manager: ObjectManagerProxy<'a>
}

impl<'a> BluezSession<'a> {
    pub async fn new() -> zbus::Result<Self> {
        let conn = Connection::system().await?;
        let adapter = AdapterProxy::new(&conn).await?;
        let object_manager = ObjectManagerProxy::builder(&conn).destination("org.bluez")?.path("/")?.build().await?;

        Ok(Self {
            connection: conn,
            adapter,
            object_manager
        })
    }

    /// make sure `path` under our adapter and it is not a subpath of a device (which would contain a '/')
    fn is_device_path(&self, path: &ObjectPath) -> bool {
        let adapter_path = self.adapter.0.path().as_str();
        path.strip_prefix(adapter_path)
            // the first character will be a '/'
            .map(|relative_path| relative_path.len() >= 1 && !&relative_path[1..].contains('/'))
            .unwrap_or(false)
    }

    /// get all known devices
    pub async fn get_devices(&self) -> zbus::Result<Vec<DiscoveredDevice>> {
        // get existing managed objects
        let objects: HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>> = self.object_manager.get_managed_objects().await?;

        // convert each item into a Device
        Ok(objects.into_iter()
           .filter_map(|(path, mut value)| {
               if self.is_device_path(&path) {
                   let mut device = value.remove("org.bluez.Device1")?;
                   let address: String = device.remove("Address")?.try_into().ok()?;
                   // convert to Vec, then collect into HashSet
                   let services = Vec::<_>::try_from(device.remove("UUIDs")?).ok()?.into_iter().collect();
                   let connected: bool = device.remove("Connected")?.try_into().ok()?;
                   // the rssi may not exist on the device hash map
                   let rssi: Option<i16> = device.remove("RSSI").and_then(|v| v.try_into().ok());
                   Some(DiscoveredDevice {
                       path,
                       address,
                       services,
                       rssi,
                       connected
                   })
               } else { None }
            })
            .collect())
    }

    /// stream device added/removed events
    pub async fn stream_device_events<'b>(&'b self) -> zbus::Result<impl futures_util::Stream<Item = DiscoveredDeviceEvent> + 'b> {
        let added_objects = self.object_manager.receive_interfaces_added().await?;
        let removed_objects = self.object_manager.receive_interfaces_removed().await?;

        let added_devices = added_objects.filter_map(move |signal: InterfacesAdded| async move {
            let args = signal.args().ok()?;
            if self.is_device_path(&args.object_path) {
                let device = args.interfaces_and_properties.get("org.bluez.Device1")?;
                let address: String = device.get("Address")?.try_into().ok()?;
                let services = Vec::<_>::try_from(device.get("UUIDs")?.try_to_owned().unwrap()).ok()?.into_iter().collect();
                let connected: bool = device.get("Connected")?.try_into().ok()?;
                let rssi: Option<i16> = device.get("RSSI").and_then(|v| v.try_into().ok());
                Some(DiscoveredDeviceEvent::DeviceAdded(DiscoveredDevice {
                    path: args.object_path.into(),
                    address,
                    services,
                    connected,
                    rssi
                }))
            } else { None }
        });

        let removed_devices = removed_objects.filter_map(move |signal| async move {
            let args = signal.args().ok()?;
            // if this is a device, and one of the removed interfaces was Device1
            if self.is_device_path(&args.object_path) && args.interfaces.contains(&"org.bluez.Device1") {
                Some(DiscoveredDeviceEvent::DeviceRemoved(args.object_path.into()))
            } else { None }
        });

        Ok(select(added_devices, removed_devices))
    }

    /// Get all services/characteristics under a device
    /// Returns a map of service UUID to map of char UUID to char proxy
    pub async fn get_device_characteristics<'b, 'c>(&self, device_path: &ObjectPath<'b>) -> zbus::Result<DeviceServiceChars<'c>> {
        // map of service UUID to object path
        let mut services = HashMap::<String, OwnedObjectPath>::new();
        // map of service object path to map of characteristic uuid to characteristic path
        let mut service_chars = HashMap::<OwnedObjectPath, HashMap::<String, GattCharacteristicProxy>>::new();

        let device_path = device_path.as_str();

        // iterate through all objects, finding the chars and services
        let objects: HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>> = self.object_manager.get_managed_objects().await?;

        for (path, mut interfaces) in objects {
            // make sure it's under this device
            if path.starts_with(device_path) {
                if let Some(mut service) = interfaces.remove("org.bluez.GattService1") {
                    // get the uuid for this service
                    if let Some(uuid) = service.remove("UUID").and_then(|a| a.try_into().ok()) {
                        services.insert(uuid, path);
                    }
                } else if let Some(mut characteristic) = interfaces.remove("org.bluez.GattCharacteristic1") {
                    if let Some(service_path) = characteristic.remove("Service").and_then(|a| a.try_into().ok()) {
                        let char_map = service_chars.entry(service_path).or_insert_with(|| HashMap::new());
                        // get the uuid for this char
                        if let Some(uuid) = characteristic.remove("UUID").and_then(|a| a.try_into().ok()) {
                            // make a connection proxy
                            if let Ok(char_proxy) = GattCharacteristicProxy::builder(&self.connection).path(path).expect("is a valid path").build().await {
                                char_map.insert(uuid, char_proxy);
                            }
                        }
                    }
                }
            }
        }

        // combine the two maps above
        Ok(services.into_iter().map(|(uuid, path)| {
            let chars = service_chars.remove(&path).unwrap_or_else(|| HashMap::new());
            (uuid, chars)
        }).collect())
    }

    pub async fn proxy_from_discovered_device<'b, 'c, 'd>(&'c self, device: &'b DiscoveredDevice) -> zbus::Result<DeviceProxy<'d>> {
        DeviceProxy::builder(&self.connection).path(device.path.to_owned()).expect("is a valid path").build().await
    }
}
