use gtk::glib::{self, Object};

use crate::bluez::DiscoveredDevice;

glib::wrapper! {
    pub struct DeviceRowObject(ObjectSubclass<imp::DeviceRowObject>);
}

impl DeviceRowObject {
    pub fn new(address: String, connected: bool, rssi: Option<i32>) -> Self {
        Object::builder()
            .property("address", address)
            .property("connected", connected)
            // todo: don't defaul to 0
            .property("rssi", rssi.unwrap_or(0))
            .build()
    }
}

impl From<DiscoveredDevice> for DeviceRowObject {
    fn from(value: DiscoveredDevice) -> Self {
        Self::new(value.address, value.connected, value.rssi.map(|v| v as i32))
    }
}

mod imp {
    use std::cell::RefCell;

    use gtk::{glib::{self, Properties}, prelude::*, subclass::prelude::*};
    
    #[derive(Default)]
    pub struct DeviceRowData {
        pub address: String,
        pub connected: bool,
        pub rssi: i32
    }
    
    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::DeviceRowObject)]
    pub struct DeviceRowObject {
        #[property(name = "address", get, set, type = String, member = address)]
        #[property(name = "connected", get, set, type = bool, member = connected)]
        #[property(name = "rssi", get, set, type = i32, member = rssi)]
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
