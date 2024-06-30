use std::collections::HashMap;

use chrono::{DateTime, Local};

use crate::{band::BatteryStatus, utils::format_date};

use super::card::{InfoItem, InfoItemType, InfoItemValue, InfoItemValues};

pub const BATTERY_ITEMS: [InfoItem<'static>; 3] = [
    InfoItem { item_type: InfoItemType::Field, id: "level", label: "Battery Level", classes: &[] },
    InfoItem { item_type: InfoItemType::Field, id: "last_charge", label: "Last Charge", classes: &[] },
    InfoItem { item_type: InfoItemType::Indicator, id: "charging", label: "Charging", classes: &["success"] },
];

pub const TIME_ITEMS: [InfoItem<'static>; 2]  = [
    InfoItem { item_type: InfoItemType::Field, id: "current_time", label: "Current Band Time", classes: &[] },
    InfoItem { item_type: InfoItemType::Button, id: "sync_time", label: "Sync Time", classes: &[] }
];

pub trait IntoInfoItemValues {
    fn into_info_item_values(self) -> InfoItemValues;
}

impl IntoInfoItemValues for BatteryStatus {
    fn into_info_item_values(self) -> InfoItemValues {
        HashMap::from([
            ("level".into(), InfoItemValue::Field(format!("{}%", self.battery_level))),
            ("last_charge".into(), InfoItemValue::Field(format_date(&self.last_charge))),
            ("charging".into(), InfoItemValue::Indicator(self.charging))
        ])
    }
}

// (current_time, authenticated)
impl IntoInfoItemValues for (DateTime<Local>, bool) {
    fn into_info_item_values(self) -> InfoItemValues {
        HashMap::from([
            ("current_time".into(), InfoItemValue::Field(format_date(&self.0))),
            // enable the button if we're authenticated
            ("sync_time".into(), InfoItemValue::Button(self.1))
        ])
    }
}
