use aes::{cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit}, Aes128};
use cbc::Encryptor;

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
