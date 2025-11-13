use image::ImageBuffer;
use image::Luma;
use crate::consts::INVERT_BITS;

/// Helper to append an EPL ASCII command line terminated with CRLF
pub fn epl_line(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.extend_from_slice(b"\r\n");
}

/// Convert a 1-bit image (Luma 0=black, 255=white) into row-packed bytes.
/// Returns (width, height, rows)
pub fn image_to_row_bytes(img: &ImageBuffer<Luma<u8>, Vec<u8>>) -> (u32, u32, Vec<u8>) {
    let w = img.width();
    let h = img.height();
    let bpr = ((w + 7) / 8) as usize;
    let mut out = vec![0u8; bpr * h as usize];

    for y in 0..h as usize {
        for x in 0..w as usize {
            let black = img.get_pixel(x as u32, y as u32).0[0] < 128;
            if black {
                let idx = y * bpr + (x / 8);
                out[idx] |= 1 << (7 - (x % 8));
            }
        }
    }
    if INVERT_BITS {
        for b in &mut out { *b = !*b; }
    }
    (w, h, out)
}

/// Append GW header + raw binary rows + CRLF
pub fn gw_bytes(buf: &mut Vec<u8>, x: u32, y: u32, w: u32, h: u32, rows: &[u8]) {
    let bpr = ((w + 7) / 8) as usize;
    epl_line(buf, &format!("GW{},{},{},{}", x, y, bpr, h));
    buf.extend_from_slice(rows);
    buf.extend_from_slice(b"\r\n");
}
use image::GrayImage;

/// Convert a 1-bit gray image (white=255, black=0) into an EPL2 GW command payload.
/// Returns bytes: ASCII header + binary image data.
pub fn image_to_gw(x: u32, y: u32, img: &GrayImage) -> Vec<u8> {
    let width = img.width();
    let height = img.height();

    // bytes per row (8 pixels per byte)
    let bytes_per_row = ((width + 7) / 8) as usize;
    let mut data: Vec<u8> = Vec::new();

    // Header: GWx,y,bytes_per_row,height (EPL expects bytes-per-row then height)
    let header = format!("GW{},{},{},{}\r\n", x, y, bytes_per_row, height);
    data.extend_from_slice(header.as_bytes());

    // Build rows properly and append raw binary bytes (MSB-first)
    let bpr = bytes_per_row;
    let mut rows: Vec<u8> = vec![0u8; bpr * height as usize];
    for row in 0..height as usize {
        for col in 0..width as usize {
            let px = img.get_pixel(col as u32, row as u32)[0];
            let is_black = px < 128;
            if is_black {
                let idx = row * bpr + (col / 8);
                let bit = 7 - (col % 8);
                rows[idx] |= 1u8 << bit;
            }
        }
    }
    data.extend_from_slice(&rows);
    // End of GW payload must be terminated with CRLF
    data.extend_from_slice(b"\r\n");

    data
}
