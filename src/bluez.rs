use std::collections::{HashMap, HashSet};

use zbus::{fdo::{ObjectManagerProxy, InterfacesAdded}, names::OwnedInterfaceName, proxy, zvariant::{DeserializeDict, ObjectPath, OwnedObjectPath, OwnedValue, SerializeDict, Type}, Connection};

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

// #region Bluez interfaces

#[proxy(default_service = "org.bluez", default_path = "/org/bluez/hci0", interface = "org.bluez.Adapter1", gen_blocking = false)]
trait Adapter {
    fn get_discovery_filters(&self) -> zbus::Result<Vec<String>>;
    fn set_discovery_filter(&self, filter: DiscoveryFilter) -> zbus::Result<()>;
    fn start_discovery(&self) -> zbus::Result<()>;
    fn stop_discovery(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn powered(&self) -> zbus::Result<bool>;
}

#[proxy(default_service = "org.bluez", interface = "org.bluez.Device1", gen_blocking = false)]
trait BluezDevice {
    fn connect_profile(&self, uuid: &str) -> zbus::Result<()>;
    fn disconnect(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn address(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn connected(&self) -> zbus::Result<bool>;
}

#[proxy(default_service = "org.bluez", interface = "org.bluez.GattService1", gen_blocking = false)]
trait GattService {
    #[zbus(property, name = "UUID")]
    fn uuid(&self) -> zbus::Result<String>;
}

#[proxy(default_service = "org.bluez", interface = "org.bluez.GattCharacteristic1", gen_blocking = false)]
trait GattCharacteristic {
    #[zbus(property, name = "UUID")]
    fn uuid(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn service(&self) -> zbus::Result<ObjectPath>;
}

// #endregion

#[derive(Clone)]
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
            .map(|relative_path| !&relative_path[1..].contains('/'))
            .unwrap_or(false)
    }

    pub async fn get_devices(&self) -> zbus::Result<Vec<Device>> {
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
                   Some(Device {
                       path,
                       address,
                       services
                   })
               } else { None }
            })
            .collect())
    }

    pub async fn stream_device_events(&'a self) -> zbus::Result<impl futures_util::Stream<Item = DeviceEvent> + 'a> {
        let added_objects = self.object_manager.receive_interfaces_added().await?;
        let removed_objects = self.object_manager.receive_interfaces_removed().await?;

        let added_devices = added_objects.filter_map(move |signal: InterfacesAdded| async move {
            let args = signal.args().ok()?;
            if self.is_device_path(&args.object_path) {
                let device = args.interfaces_and_properties.get("org.bluez.Device1")?;
                let address: String = device.get("Address")?.try_into().ok()?;
                let services = Vec::<_>::try_from(device.get("UUIDs")?.try_to_owned().unwrap()).ok()?.into_iter().collect();
                Some(DeviceEvent::DeviceAdded(Device {
                    path: args.object_path.into(),
                    address,
                    services
                }))
            } else { None }
        });

        let removed_devices = removed_objects.filter_map(move |signal| async move {
            let args = signal.args().ok()?;
            // if this is a device, and one of the removed interfaces was Device1
            if self.is_device_path(&args.object_path) && args.interfaces.contains(&"org.bluez.Device1") {
                Some(DeviceEvent::DeviceRemoved(args.object_path.into()))
            } else { None }
        });

        Ok(select(added_devices, removed_devices))
    }

    pub async fn get_device_characteristics<'b>(&self, device_path: ObjectPath<'b>) -> zbus::Result<()> {
        // map of service UUID to object path
        let services = HashMap::<String, ObjectPath>::new();
        // mpa of service object path
        Ok(())
    }
}

#[derive(Debug)]
pub enum DeviceEvent {
    DeviceAdded(Device),
    DeviceRemoved(OwnedObjectPath)
}

#[derive(Debug, Clone)]
pub struct Device {
    pub path: OwnedObjectPath,
    pub address: String,
    pub services: HashSet<String>
}
