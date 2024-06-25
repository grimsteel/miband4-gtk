use zbus::{proxy, zvariant::{DeserializeDict, SerializeDict, Type}};

#[derive(DeserializeDict, SerializeDict, Type)]
#[zvariant(signature = "dict")]
pub struct DiscoveryFilter {
    pub uuids: Vec<String>,
    pub duplicate_data: bool,
    pub pattern: String,
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
