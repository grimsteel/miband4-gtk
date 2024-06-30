use std::fmt::Display;

use aes::{cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit}, Aes128};
use cbc::Encryptor;
use chrono::{DateTime, TimeZone};

pub const APP_ID: &'static str = "me.grimsteel.miband4-gtk";

pub fn decode_hex(hex_string: &str) -> Option<Vec<u8>> {
    // make sure it's not odd
    if hex_string.len() & 0b1 == 0b1 { return None; }
    
    (0..hex_string.len())
        .step_by(2)
        .map(|idx| u8::from_str_radix(&hex_string[idx..idx + 2], 16))
        .collect::<Result<Vec<_>, _>>().ok()
}

pub fn encrypt_value(key: &[u8], value: &[u8]) -> Option<[u8; 48]> {
    // blank iv
    let iv = [0x00; 16];
    let encryptor = Encryptor::<Aes128>::new(key.into(), &iv.into());
    let mut buf = [0x00; 48];
    encryptor.encrypt_padded_b2b_mut::<Pkcs7>(value, &mut buf).ok()?;
    Some(buf)
}

pub fn is_hex_string(string: &str) -> bool {
    string.chars().all(|c| (c >= '0' && c <= '9') || (c >= 'A' && c <= 'F') || (c >= 'a' && c <= 'f'))
}

pub fn format_date<T: TimeZone<Offset: Display>>(date: &DateTime<T>) -> String {
    format!("{}", date.format("%m/%d/%y %I:%M %p"))
}

/// returns the equivalent distance in feet or miles
pub fn meters_to_imperial(meters: u16) -> String {
    // below 0.1 miles (528 feet, 161 meters), display in feet
    if meters < 161 {
        format!("{:.3} ft", (meters as f64) * 3.281)
    } else {
        format!("{:.3} mi", (meters as f64) / 1609.344)
    }
}
