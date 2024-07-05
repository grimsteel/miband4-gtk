use std::collections::HashSet;

use gtk::glib::{self, Object};
use zbus::zvariant::OwnedObjectPath;

use crate::bluez::DiscoveredDevice;

glib::wrapper! {
    pub struct DeviceRowObject(ObjectSubclass<imp::DeviceRowObject>);
}

impl DeviceRowObject {
    pub fn new(address: String, connected: bool, rssi: Option<i32>, path: String, alias: String) -> Self {
        Object::builder()
            .property("address", address)
            .property("alias", alias)
            .property("connected", connected)
            .property("rssi", rssi.unwrap_or(0))
            .property("path", path)
            .build()
    }
}

// Device, Alias
impl From<(DiscoveredDevice, String)> for DeviceRowObject {
    fn from((value, alias): (DiscoveredDevice, String)) -> Self {
        Self::new(value.address, value.connected, value.rssi.map(|v| v as i32), value.path.as_str().into(), alias)
    }
}

impl From<DeviceRowObject> for DiscoveredDevice {
    fn from(value: DeviceRowObject) -> Self {
        let rssi = value.rssi() as i16;
        Self {
            path: OwnedObjectPath::try_from(value.path()).unwrap(),
            connected: value.connected(),
            rssi: if rssi == 0 { None } else { Some(rssi) },
            address: value.address(),
            // we don't store services (they aren't needed apart from filtering a list of DiscoveredDevices)
            services: HashSet::new()
        }
    }
}

mod imp {
    use std::cell::RefCell;

    use gtk::{glib::{self, Properties}, prelude::*, subclass::prelude::*};
    
    #[derive(Default)]
    pub struct DeviceRowData {
        pub address: String,
        pub connected: bool,
        pub rssi: i32,
        pub path: String,
        pub alias: String
    }
    
    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::DeviceRowObject)]
    pub struct DeviceRowObject {
        #[property(name = "address", get, set, type = String, member = address)]
        #[property(name = "path", get, set, type = String, member = path)]
        #[property(name = "connected", get, set, type = bool, member = connected)]
        #[property(name = "rssi", get, set, type = i32, member = rssi)]
        #[property(name = "alias", get, set, type = String, member = alias)]
        pub data: RefCell<DeviceRowData>
    }

    #[glib::object_subclass]
    impl ObjectSubclass for DeviceRowObject {
        const NAME: &'static str = "MiBand4DeviceRowObject";
        type Type = super::DeviceRowObject;
    }

    #[glib::derived_properties]
    impl ObjectImpl for DeviceRowObject {}
}
