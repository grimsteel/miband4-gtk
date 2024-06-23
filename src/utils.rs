pub fn decode_hex(hex_string: &str) -> Option<Vec<u8>> {
    // make sure it's not odd
    if hex_string.len() & 0b1 == 0b1 { return None; }
    
    (0..hex_string.len())
        .step_by(2)
        .map(|idx| u8::from_str_radix(&hex_string[idx..idx + 2], 16))
        .collect::<Result<Vec<_>, _>>().ok()
}
