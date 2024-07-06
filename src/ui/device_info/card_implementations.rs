use std::collections::HashMap;

use chrono::{DateTime, Local};

use crate::{band::{BatteryStatus, CurrentActivity, MiBand}, store::{ActivityGoal, BandLock}, utils::{format_date, meters_to_imperial}};

use super::card::{InfoItem, InfoItemType, InfoItemValue, InfoItemValues};

pub const BATTERY_ITEMS: [InfoItem<'static>; 3] = [
    InfoItem { item_type: InfoItemType::Field, id: "level", label: "Battery Level", classes: &[] },
    InfoItem { item_type: InfoItemType::Field, id: "last_charge", label: "Last Charge", classes: &[] },
    InfoItem { item_type: InfoItemType::Indicator, id: "charging", label: "Charging", classes: &["success"] },
];

pub const TIME_ITEMS: [InfoItem<'static>; 2] = [
    InfoItem { item_type: InfoItemType::Field, id: "current_time", label: "Current Band Time", classes: &[] },
    InfoItem { item_type: InfoItemType::Button, id: "sync_time", label: "Sync Time", classes: &[] }
];

pub const DEVICE_INFO_ITEMS: [InfoItem<'static>; 4] = [
    InfoItem { item_type: InfoItemType::Field, id: "mac", label: "MAC Address", classes: &[] },
    InfoItem { item_type: InfoItemType::Field, id: "firmware_version", label: "Firmware Version", classes: &[] },
    InfoItem { item_type: InfoItemType::Field, id: "dbus_path", label: "D-Bus Path", classes: &[] },
    InfoItem { item_type: InfoItemType::Button, id: "disconnect", label: "Disconnect", classes: &[] }
];

pub const ACTIVITY_ITEMS: [InfoItem<'static>; 3] = [
    InfoItem { item_type: InfoItemType::Field, id: "steps", label: "Steps", classes: &[] },
    InfoItem { item_type: InfoItemType::Field, id: "distance", label: "Distance", classes: &[] },
    InfoItem { item_type: InfoItemType::Field, id: "calories", label: "Calories Burned", classes: &[] }
];

pub const ACTIVITY_GOAL_ITEMS: [InfoItem<'static>; 3] = [
    InfoItem { item_type: InfoItemType::Entry, id: "steps", label: "Step Goal", classes: &[] },
    InfoItem { item_type: InfoItemType::Switch, id: "notifications", label: "Goal Notifications", classes: &[] },
    InfoItem { item_type: InfoItemType::Button, id: "save_goal", label: "Save", classes: &[] }
];

pub const BAND_LOCK_ITEMS: [InfoItem<'static>; 3] = [
    InfoItem { item_type: InfoItemType::Switch, id: "lock_enabled", label: "Enable Band Lock", classes: &[] },
    InfoItem { item_type: InfoItemType::Entry, id: "lock_pin", label: "Band PIN", classes: &[] },
    InfoItem { item_type: InfoItemType::Button, id: "save_band_lock", label: "Save", classes: &[] }
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

impl IntoInfoItemValues for CurrentActivity {
    fn into_info_item_values(self) -> InfoItemValues {
        HashMap::from([
            ("steps".into(), InfoItemValue::Field(self.steps.to_string())),
            ("distance".into(), InfoItemValue::Field(meters_to_imperial(self.meters))),
            ("calories".into(), InfoItemValue::Field(self.calories.to_string()))
        ])
    }
}

// (device, firmware_revision)
impl<'a> IntoInfoItemValues for (&MiBand<'a>, String) {
    fn into_info_item_values(self) -> InfoItemValues {
        HashMap::from([
            ("mac".into(), InfoItemValue::Field(self.0.address.clone())),
            ("firmware_version".into(), InfoItemValue::Field(self.1)),
            ("dbus_path".into(), InfoItemValue::Field(self.0.path().as_str().to_string())),
            ("disconnect".into(), InfoItemValue::Button(true))
        ])
    }
}

impl IntoInfoItemValues for &ActivityGoal {
    fn into_info_item_values(self) -> InfoItemValues {
        HashMap::from([
            ("steps".into(), InfoItemValue::Entry(self.steps.to_string())),
            ("notifications".into(), InfoItemValue::Switch(self.notifications)),
            // always enabled
            ("save_goal".into(), InfoItemValue::Button(true))
        ])
    }
}

impl From<InfoItemValues> for ActivityGoal {
    fn from(values: InfoItemValues) -> Self {
        Self {
            steps: values.get("steps")
            // get the steps value and parse it as a u16
                .and_then(|v| if let InfoItemValue::Entry(val) = v { val.trim().parse().ok() } else { None })
                .unwrap_or_default(),
            notifications: values.get("notifications")
            // get the bool out of the switch
                .and_then(|v| if let InfoItemValue::Switch(val) = v { Some(*val) } else { None })
                .unwrap_or_default(),
        }
    }
}

impl IntoInfoItemValues for &BandLock {
    fn into_info_item_values(self) -> InfoItemValues {
        HashMap::from([
            ("lock_enabled".into(), InfoItemValue::Switch(self.enabled)),
            ("lock_pin".into(), InfoItemValue::Entry(self.pin.clone())),
            ("save_band_lock".into(), InfoItemValue::Button(true))
        ])
    }
}

impl From<InfoItemValues> for BandLock {
    fn from(values: InfoItemValues) -> Self {
        Self {
            enabled: values.get("lock_enabled")
            // get the bool out of the switch
                .and_then(|v| if let InfoItemValue::Switch(val) = v { Some(*val) } else { None })
                .unwrap_or_default(),
            pin: values.get("lock_pin")
            // get the string out of the trny
                .and_then(|v| if let InfoItemValue::Entry(val) = v { Some(val.clone()) } else { None })
                .unwrap_or_default()
        }
    }
}
