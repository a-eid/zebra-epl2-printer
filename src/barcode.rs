/// Simple EAN-13 checksum and normalization helper
pub fn normalize_ean13(mut code: String) -> Result<String, String> {
    // remove non-digits
    code.retain(|c| c.is_ascii_digit());
    if code.len() == 12 {
        let check = compute_ean13_checksum(&code)?;
        code.push(char::from_digit(check as u32, 10).unwrap());
        Ok(code)
    } else if code.len() == 13 {
        // validate
        let prefix = code[..12].to_string();
        let expected = compute_ean13_checksum(&prefix)?;
        let last = code.chars().last().unwrap().to_digit(10).unwrap() as u8;
        if expected == last {
            Ok(code)
        } else {
            Err("invalid checksum".into())
        }
    } else {
        Err("barcode must have 12 or 13 digits".into())
    }
}

fn compute_ean13_checksum(digits: &str) -> Result<u8, String> {
    if digits.len() != 12 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err("EAN13 checksum requires 12 digits".into());
    }
    let mut sum = 0u32;
    for (i, ch) in digits.chars().enumerate() {
        let d = ch.to_digit(10).unwrap();
        if (i % 2) == 0 {
            sum += d;
        } else {
            sum += d * 3;
        }
    }
    let modulo = sum % 10;
    let check = if modulo == 0 { 0 } else { 10 - modulo };
    Ok(check as u8)
}
