use std::collections::{HashMap, HashSet};

use futures::stream::FilterMap;
use zbus::{fdo::{ObjectManagerProxy, InterfacesAddedStream, InterfacesAdded}, names::OwnedInterfaceName, proxy, zvariant::{Array, DeserializeDict, ObjectPath, OwnedObjectPath, OwnedValue, SerializeDict, Type}, Connection};

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

#[proxy(default_service = "org.bluez", default_path = "/org/bluez/hci0", interface = "org.bluez.Adapter1", gen_blocking = false)]
trait Adapter {
    fn get_discovery_filters(&self) -> zbus::Result<Vec<String>>;
    fn set_discovery_filter(&self, filter: DiscoveryFilter) -> zbus::Result<()>;
    fn start_discovery(&self) -> zbus::Result<()>;
    fn stop_discovery(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn powered(&self) -> zbus::Result<bool>;
}

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

    pub async fn get_devices(&self) -> zbus::Result<Vec<Device>> {
        let objects: HashMap<OwnedObjectPath, HashMap<OwnedInterfaceName, HashMap<String, OwnedValue>>> = self.object_manager.get_managed_objects().await?;
        let adapter_path = self.adapter.0.path().as_str();
        Ok(objects.into_iter()
           .filter_map(|(path, mut value)| {
               // make sure it's under our adapter and it is not a subpath of a device (which would contain a '/')
               if path.strip_prefix(adapter_path)
                   .and_then(|p| p.strip_prefix('/'))
                   .map(|relative_path| !relative_path.contains('/'))
                   .unwrap_or(false)
               {
                   let mut device = value.remove("org.bluez.Device1")?;
                   let address: String = device.remove("Address")?.try_into().ok()?;
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

    pub async fn stream_new_devices(&'a self) -> zbus::Result<impl futures_util::Stream + 'a> {
        let objects: InterfacesAddedStream = self.object_manager.receive_interfaces_added().await?;

        let adapter_path = self.adapter.0.path().as_str();
        Ok(objects.filter_map(move |signal: InterfacesAdded| async move {
            let args = signal.args().ok()?;
            if args.object_path.strip_prefix(adapter_path)
                .and_then(|p| p.strip_prefix('/'))
                .map(|relative_path| !relative_path.contains('/'))
                .unwrap_or(false)
            {
                let device = args.interfaces_and_properties.get("org.bluez.Device1")?;
                let address: String = device.get("Address")?.try_into().ok()?;
                let services = Vec::<_>::try_from(device.get("UUIDs")?.try_to_owned().unwrap()).ok()?.into_iter().collect();
                Some(Device {
                    path: args.object_path.into(),
                    address,
                    services
                })
            } else { None }
        }))
    }
}

#[derive(Debug, Clone)]
pub struct Device {
    pub path: OwnedObjectPath,
    pub address: String,
    pub services: HashSet<String>
}
