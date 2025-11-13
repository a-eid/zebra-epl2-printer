const LABEL_W: u32 = 440;
const LABEL_H: u32 = 320;
const PAD_RIGHT: u32 = 10;
const FONT_PX: f32 = 42.0;   // bigger
const DARKNESS: u8 = 5;      // lighter to reduce banding
const SPEED: u8 = 3;
const NARROW: u32 = 2;       // 2..3
const HEIGHT: u32 = 50;
const LANDSCAPE: bool = false; // set true only if your driver forces rotation

use crate::graphics::{render_arabic_line_tight_1bit, rotate90};
use crate::epl::{epl_line, image_to_row_bytes, gw_bytes};

fn center_x_for_ean13(label_w: u32, narrow: u32) -> u32 {
    let modules = 95i32; // EAN-13 modules
    let w = modules * narrow as i32;
    ((label_w as i32 - w) / 2).max(0) as u32
}

pub fn build_two_product_label_clean_centered(
    font_bytes: &[u8],
    p1_name: &str, p1_price: &str, p1_barcode: &str,
    p2_name: &str, p2_price: &str, p2_barcode: &str,
) -> Vec<u8> {
    // Arabic + currency
    let t1 = format!("{}    {} {}", p1_name, p1_price, "ج.م");
    let t2 = format!("{}    {} {}", p2_name, p2_price, "ج.م");

    // Render as tight images (just glyph width) to avoid heating wide empty area
    let mut im1 = render_arabic_line_tight_1bit(&t1, font_bytes, FONT_PX, 3);
    let mut im2 = render_arabic_line_tight_1bit(&t2, font_bytes, FONT_PX, 3);
    if LANDSCAPE { im1 = rotate90(&im1); im2 = rotate90(&im2); }

    let (w1,h1,r1) = image_to_row_bytes(&im1);
    let (w2,h2,r2) = image_to_row_bytes(&im2);

    // Right-align x = LABEL_W − PAD_RIGHT − w
    let x1 = LABEL_W - PAD_RIGHT - w1;
    let x2 = LABEL_W - PAD_RIGHT - w2;

    // Y positions (ensure HRI fits)
    let text1_y = 8;
    let bc1_y   = text1_y + h1 + 16;
    let text2_y = bc1_y   + HEIGHT + 26;
    let bc2_y   = text2_y + h2 + 16;

    let bx_center = center_x_for_ean13(LABEL_W, NARROW);

    let mut buf = Vec::new();
    epl_line(&mut buf, "N");
    epl_line(&mut buf, &format!("q{}", LABEL_W));
    epl_line(&mut buf, &format!("Q{},24", LABEL_H));
    epl_line(&mut buf, &format!("D{}", DARKNESS));
    epl_line(&mut buf, &format!("S{}", SPEED));

    if !LANDSCAPE {
        gw_bytes(&mut buf, x1, text1_y, w1, h1, &r1);
        epl_line(&mut buf, &format!("B{},{},0,1,{},{},{},B,\"{}\"",
            bx_center, bc1_y, NARROW, 4, HEIGHT, p1_barcode));

        gw_bytes(&mut buf, x2, text2_y, w2, h2, &r2);
        epl_line(&mut buf, &format!("B{},{},0,1,{},{},{},B,\"{}\"",
            bx_center, bc2_y, NARROW, 4, HEIGHT, p2_barcode));
    } else {
        // If you must compensate for landscape, place with swapped coords
        gw_bytes(&mut buf, text1_y, x1, w1, h1, &r1);
        epl_line(&mut buf, &format!("B{},{},1,1,{},{},{},B,\"{}\"",
            bc1_y, bx_center, NARROW, 4, HEIGHT, p1_barcode));

        gw_bytes(&mut buf, text2_y, x2, w2, h2, &r2);
        epl_line(&mut buf, &format!("B{},{},1,1,{},{},{},B,\"{}\"",
            bc2_y, bx_center, NARROW, 4, HEIGHT, p2_barcode));
    }

    epl_line(&mut buf, "P1");
    buf
}
