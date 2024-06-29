use gtk::glib::{self, Object};

use crate::{band::BatteryStatus, utils::format_date};

use super::card::{DeviceInfoCard, InfoItem, InfoItemType};

glib::wrapper! {
    pub struct Battery(ObjectSubclass<imp::Battery>);
}

const CARD_ITEMS: [InfoItem<'static>; 1] = [
    InfoItem { item_type: InfoItemType::Field, id: "level", label: "Battery Level", classes: &[] }
];

impl Battery {
    pub fn new() -> Self { Object::builder().build() }

    pub fn set_loading(&self) {
        self.set_level("Loading...");
        self.set_last_charge("Loading...");
        self.set_charging(false);
    }

    pub fn update_from_battery_status(&self, status: &BatteryStatus) {
        self.set_level(format!("{}%", status.battery_level));
        self.set_last_charge(format_date(&status.last_charge));
        self.set_charging(status.charging);
    }

    pub fn bind_to_card(&self, card: &DeviceInfoCard) {
        card.handle_items(&CARD_ITEMS);
        card.set_values(self.clone());
    }
}

impl Default for Battery {
    fn default() -> Self {
        Self::new()
    }
}

mod imp {
    use std::{cell::RefCell, sync::atomic::AtomicBool};

    use gtk::{glib::{self, Properties}, prelude::*, subclass::prelude::*};
    
    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::Battery)]
    pub struct Battery {
        #[property(get, set)]
        pub level: RefCell<String>,
        #[property(get, set)]
        pub last_charge: RefCell<String>,
        #[property(get, set)]
        pub charging: AtomicBool
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Battery {
        const NAME: &'static str = "MiBand4Battery";
        type Type = super::Battery;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Battery {}
}
