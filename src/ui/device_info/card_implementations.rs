use std::collections::HashMap;

use crate::{band::BatteryStatus, utils::format_date};

use super::card::{InfoItem, InfoItemType, InfoItemValue};

pub const BATTERY_ITEMS: [InfoItem<'static>; 3] = [
    InfoItem { item_type: InfoItemType::Field, id: "level", label: "Battery Level", classes: &[] },
    InfoItem { item_type: InfoItemType::Field, id: "last_charge", label: "Last Charge", classes: &[] },
    InfoItem { item_type: InfoItemType::Indicator, id: "charging", label: "Charging", classes: &["success"] },
];

impl<'a> From<BatteryStatus> for HashMap<String, InfoItemValue> {
    fn from(value: BatteryStatus) -> Self {
        Self::from([
            ("level".into(), InfoItemValue::Field(format!("{}%", value.battery_level))),
            ("last_charge".into(), InfoItemValue::Field(format_date(&value.last_charge))),
            ("charging".into(), InfoItemValue::Indicator(value.charging))
        ])
    }
}
