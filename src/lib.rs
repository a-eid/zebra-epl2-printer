//! Self-contained EPL2 label builder for Zebra LP-2824 (203 dpi).
//! - Fixes Arabic shaping + direction
//! - Renders tight 1-bit bitmaps (no gray strip)
//! - Optional bit inversion for GW polarity
//! - Compensates driver Landscape by rotating in code
//! - Centers EAN-13 barcodes and keeps HRI visible

use image::{ImageBuffer, Luma};
use rusttype::{Font, Scale, point};
use ar_reshaper::{ArabicReshaper, ReshaperConfig};
use unicode_bidi::BidiInfo;

// ======== Config (edit if needed) ========

const LABEL_W: u32 = 440;          // dots (≈55 mm)
const LABEL_H: u32 = 320;          // dots (≈40 mm)

const FONT_PX: f32 = 36.0;         // larger for better readability in 4-product layout
const BOLD_STROKE: bool = true;    // draw twice w/ 1px offset

const DARKNESS: u8 = 8;            // D0..D15 (darker for better contrast like reference)
const SPEED: u8 = 2;               // S1..S6 (slower for better quality)

const NARROW: u32 = 2;             // EAN13 module width (back to 2 like reference)
const HEIGHT: u32 = 35;            // barcode bar height (smaller for 4-product layout)

const INVERT_BITS: bool = true;      // Invert GW bits for black-on-white

// ======== Public API ========

/// Build a single EPL2 print job for two products (original working implementation).
/// - `font_bytes`: embedded Arabic font bytes 
/// - `name1/price1/barcode1` + `name2/price2/barcode2`
/// Returns raw bytes ready to send to the printer (USB raw write).
pub fn build_two_product_label_with_brand(
    font_bytes: &[u8],
    brand: &str,
    name1: &str, price1: &str, barcode1: &str,
    name2: &str, price2: &str, barcode2: &str,
) -> Vec<u8> {
    // Ensure barcodes are valid EAN-13 format
    let bc1 = ensure_valid_ean13(barcode1);
    let bc2 = ensure_valid_ean13(barcode2);

    // Render brand (large, extra bold)
    let brand_img = {
        let font = rusttype::Font::try_from_bytes(font_bytes).expect("bad font");
        let reshaper = ar_reshaper::ArabicReshaper::new(ar_reshaper::ReshaperConfig::default());
        let visual = bidi_then_shape(brand, &reshaper);
        let scale = rusttype::Scale { x: 40.0, y: 40.0 };
        let vm = font.v_metrics(scale);
        let ascent = vm.ascent.ceil();
        let descent = vm.descent.floor();
        let line_h = (ascent - descent).ceil().max(30.0) as u32;
        let glyphs: Vec<_> = font.layout(&visual, scale, rusttype::point(0.0, ascent)).collect();
        let text_w = glyphs.iter().rev()
            .find_map(|g| g.pixel_bounding_box().map(|bb| bb.max.x as f32))
            .unwrap_or(0.0).ceil() as u32;
        let w = (text_w + 4).max(2);
        let mut img = image::ImageBuffer::from_pixel(w, line_h, Luma([255]));
        let passes: &[(i32,i32)] = &[(0,0),(1,0),(2,0),(0,1)]; // quad-draw for extra boldness
        for &(_dx, _dy) in passes {
            for g in font.layout(&visual, scale, rusttype::point(2.0 + _dx as f32, ascent + _dy as f32)) {
                if let Some(bb) = g.pixel_bounding_box() {
                    g.draw(|x, y, v| {
                        if v > 0.5 { // Lower threshold for crisper rendering (was 0.65)
                            let px = x + bb.min.x as u32;
                            let py = y + bb.min.y as u32;
                            if px < w && py < line_h { img.put_pixel(px, py, Luma([0])); }
                        }
                    });
                }
            }
        }
        img
    };
    let (brand_w, brand_h, brand_r) = image_to_row_bytes(&brand_img);

    // Render product lines with space-between layout (name right, price left)
    let max_product_width = LABEL_W - 20; // Leave some padding
    let (w1, h1, r1) = render_name_price_space_between(name1, price1, font_bytes, 52.0, max_product_width, BOLD_STROKE);
    let (w2, h2, r2) = render_name_price_space_between(name2, price2, font_bytes, 52.0, max_product_width, BOLD_STROKE);

    // Layout: two vertical halves
    let half_h = LABEL_H / 2;  // 160 dots per half

    // Center brand horizontally in each half
    let brand_x = (LABEL_W - brand_w) / 2;
    let brand_y1 = 8;  // shifted up by 2px (was 10)
    let brand_y2 = half_h + 8;  // shifted up by 2px (was half_h + 10)

    // Center product text horizontally
    let x1 = (LABEL_W - w1) / 2;
    let x2 = (LABEL_W - w2) / 2;

    // Move content down to make space for brand, but reduce gap
    let brand_to_text_gap: i32 = -6; // further tighten: negative gap pulls product info closer to brand
    let row_gap: i32 = 4; // 4px between the two rows
    let text1_y = (brand_y1 as i32 + brand_h as i32 + brand_to_text_gap).max(0) as u32;
    let bc1_y = (text1_y as i32 + h1 as i32 + 4).max(0) as u32;  // reduced gap by 4px (was 8)
    let text2_y = (brand_y2 as i32 + brand_h as i32 + brand_to_text_gap + row_gap).max(0) as u32;
    let bc2_y = (text2_y as i32 + h2 as i32 + 4).max(0) as u32;  // reduced gap by 4px (was 8)

    let bx_center = center_x_for_ean13_single(LABEL_W, NARROW);

    let mut buf = Vec::new();
    epl_line(&mut buf, "N");
    epl_line(&mut buf, &format!("q{}", LABEL_W));
    epl_line(&mut buf, &format!("Q{},{}", LABEL_H, 24));
    epl_line(&mut buf, &format!("D{}", DARKNESS));
    epl_line(&mut buf, &format!("S{}", SPEED));

    // Top half
    gw_bytes(&mut buf, brand_x, brand_y1, brand_w, brand_h, &brand_r);
    gw_bytes(&mut buf, x1, text1_y, w1, h1, &r1);
    epl_line(&mut buf, &format!("B{},{},0,E30,{},{},{},B,\"{}\"",
        bx_center, bc1_y, NARROW, 3, HEIGHT, bc1));

    // Bottom half
    gw_bytes(&mut buf, brand_x, brand_y2, brand_w, brand_h, &brand_r);
    gw_bytes(&mut buf, x2, text2_y, w2, h2, &r2);
    epl_line(&mut buf, &format!("B{},{},0,E30,{},{},{},B,\"{}\"",
        bx_center, bc2_y, NARROW, 3, HEIGHT, bc2));

    epl_line(&mut buf, "P1");
    buf
}

/// Build a single EPL2 print job for four products in 2x2 grid.
/// - `font_bytes`: embedded Arabic font bytes 
/// - Four sets of `name/price/barcode` for each quadrant
/// Returns raw bytes ready to send to the printer (USB raw write).
pub fn build_four_product_label_with_brand(
    font_bytes: &[u8],
    brand: &str,
    name1: &str, price1: &str, barcode1: &str,
    name2: &str, price2: &str, barcode2: &str,
    name3: &str, price3: &str, barcode3: &str,
    name4: &str, price4: &str, barcode4: &str,
) -> Vec<u8> {
    // Ensure barcodes are valid EAN-13 format
    let bc1 = ensure_valid_ean13(barcode1);
    let bc2 = ensure_valid_ean13(barcode2);
    let bc3 = ensure_valid_ean13(barcode3);
    let bc4 = ensure_valid_ean13(barcode4);

    // Render brand (extra bold, large size) with quad-draw for extra boldness
    let brand_img = {
        let font = rusttype::Font::try_from_bytes(font_bytes).expect("bad font");
        let reshaper = ar_reshaper::ArabicReshaper::new(ar_reshaper::ReshaperConfig::default());
        let visual = bidi_then_shape(brand, &reshaper);
        let scale = rusttype::Scale { x: 40.0, y: 40.0 };
        let vm = font.v_metrics(scale);
        let ascent = vm.ascent.ceil();
        let descent = vm.descent.floor();
        let line_h = (ascent - descent).ceil().max(30.0) as u32;
        let glyphs: Vec<_> = font.layout(&visual, scale, rusttype::point(0.0, ascent)).collect();
        let text_w = glyphs.iter().rev()
            .find_map(|g| g.pixel_bounding_box().map(|bb| bb.max.x as f32))
            .unwrap_or(0.0).ceil() as u32;
        let w = (text_w + 4).max(2);
        let mut img = image::ImageBuffer::from_pixel(w, line_h, Luma([255]));
        let passes: &[(i32,i32)] = &[(0,0),(1,0),(2,0),(0,1)]; // quad-draw for extra boldness
        for &(_dx, _dy) in passes {
            for g in font.layout(&visual, scale, rusttype::point(2.0 + _dx as f32, ascent + _dy as f32)) {
                if let Some(bb) = g.pixel_bounding_box() {
                    g.draw(|x, y, v| {
                        if v > 0.5 { // Lower threshold for crisper rendering (was 0.65)
                            let px = x + bb.min.x as u32;
                            let py = y + bb.min.y as u32;
                            if px < w && py < line_h { img.put_pixel(px, py, Luma([0])); }
                        }
                    });
                }
            }
        }
        img
    };
    let (brand_w, brand_h, brand_r) = image_to_row_bytes(&brand_img);

    // Equal quadrants: 440÷2=220 width, 320÷2=160 height per quadrant
    let quad_w = LABEL_W / 2;  // 220 dots per column
    let quad_h = LABEL_H / 2;  // 160 dots per row
    let gap: i32 = -2;         // Horizontal gap between quadrants (negative to overlap slightly, reducing space by 6px from original 4)
    let grid_offset_y = 18;    // Move entire grid down (shifted up by 2px from 20)
    
    // Render product lines with space-between layout (name right, price left)
    let max_product_width = ((quad_w as i32 - gap/2 - 10).max(0)) as u32; // Quadrant width minus padding
    let (w1, h1, r1) = render_name_price_space_between(name1, price1, font_bytes, FONT_PX, max_product_width, BOLD_STROKE);
    let (w2, h2, r2) = render_name_price_space_between(name2, price2, font_bytes, FONT_PX, max_product_width, BOLD_STROKE);
    let (w3, h3, r3) = render_name_price_space_between(name3, price3, font_bytes, FONT_PX, max_product_width, BOLD_STROKE);
    let (w4, h4, r4) = render_name_price_space_between(name4, price4, font_bytes, FONT_PX, max_product_width, BOLD_STROKE);
    
    // Quadrant boundaries with gap:
    // Left column: 0 to (220-gap/2), Right column: (220+gap/2) to 440
    // Top row: grid_offset_y to (160-gap/2+offset), Bottom row: (160+gap/2+offset) to 320
    
    // Center brand horizontally in each quadrant
    let brand_x_left = ((quad_w as i32 - gap/2 - brand_w as i32) / 2).max(0) as u32;
    let brand_x_right = (quad_w as i32 + gap/2 + (quad_w as i32 - brand_w as i32) / 2).max(0) as u32;
    let brand_y_top = grid_offset_y + 4;
    let brand_y_bottom = (grid_offset_y as i32 + quad_h as i32 + gap/2 + 4).max(0) as u32;

    // Center product text horizontally within each quadrant
    let x1 = ((quad_w as i32 - gap/2 - w1 as i32) / 2).max(0) as u32;
    let x2 = (quad_w as i32 + gap/2 + (quad_w as i32 - w2 as i32) / 2).max(0) as u32;
    let x3 = ((quad_w as i32 - gap/2 - w3 as i32) / 2).max(0) as u32;
    let x4 = (quad_w as i32 + gap/2 + (quad_w as i32 - w4 as i32) / 2).max(0) as u32;

    // Content vertical positions: brand at top, then product, then barcode
    // Shift content up by 10px for better balance
    let shift_up = 10;
    let text1_y = brand_y_top + brand_h + 6 - shift_up;
    let bc1_y = text1_y + h1 + 3;

    let text2_y = brand_y_top + brand_h + 6 - shift_up;
    let bc2_y = text2_y + h2 + 3;

    let text3_y = brand_y_bottom + brand_h + 6 - shift_up;
    let bc3_y = text3_y + h3 + 3;

    let text4_y = brand_y_bottom + brand_h + 6 - shift_up;
    let bc4_y = text4_y + h4 + 3;

    let bc_left_x = (center_x_for_ean13_column(((quad_w as i32 - gap/2).max(0)) as u32, NARROW) as i32 + 4).max(0) as u32;
    let bc_right_x = (quad_w as i32 + gap/2 + center_x_for_ean13_column(((quad_w as i32 - gap/2).max(0)) as u32, NARROW) as i32).max(0) as u32;

    let mut buf = Vec::<u8>::new();
    epl_line(&mut buf, "N");
    epl_line(&mut buf, &format!("q{}", LABEL_W));
    epl_line(&mut buf, &format!("Q{},{}", LABEL_H, 24));
    epl_line(&mut buf, &format!("D{}", DARKNESS));
    epl_line(&mut buf, &format!("S{}", SPEED));

    // Top row: Brand, Product 1 (left) and Product 2 (right)
    gw_bytes(&mut buf, brand_x_left, brand_y_top, brand_w, brand_h, &brand_r);
    gw_bytes(&mut buf, brand_x_right, brand_y_top, brand_w, brand_h, &brand_r);
    gw_bytes(&mut buf, x1, text1_y, w1, h1, &r1);
    epl_line(&mut buf, &format!("B{},{},0,E30,{},{},{},B,\"{}\"",
        bc_left_x, bc1_y, NARROW, 3, HEIGHT, bc1));
    gw_bytes(&mut buf, x2, text2_y, w2, h2, &r2);
    epl_line(&mut buf, &format!("B{},{},0,E30,{},{},{},B,\"{}\"",
        bc_right_x, bc2_y, NARROW, 3, HEIGHT, bc2));

    // Bottom row: Brand, Product 3 (left) and Product 4 (right)
    gw_bytes(&mut buf, brand_x_left, brand_y_bottom, brand_w, brand_h, &brand_r);
    gw_bytes(&mut buf, brand_x_right, brand_y_bottom, brand_w, brand_h, &brand_r);
    gw_bytes(&mut buf, x3, text3_y, w3, h3, &r3);
    epl_line(&mut buf, &format!("B{},{},0,E30,{},{},{},B,\"{}\"",
        bc_left_x, bc3_y, NARROW, 3, HEIGHT, bc3));
    gw_bytes(&mut buf, x4, text4_y, w4, h4, &r4);
    epl_line(&mut buf, &format!("B{},{},0,E30,{},{},{},B,\"{}\"",
        bc_right_x, bc4_y, NARROW, 3, HEIGHT, bc4));

    epl_line(&mut buf, "P1");  // Print exactly ONE label
    buf
}

// ======== Arabic rendering ========

/// Visual-order string: BiDi runs; reshape only RTL runs.
fn bidi_then_shape(text: &str, reshaper: &ArabicReshaper) -> String {
    let info = BidiInfo::new(text, None);
    let para = &info.paragraphs[0];
    let (levels, ranges) = info.visual_runs(para, para.range.clone());

    let mut out = String::new();
    // Visual order runs; reshape RTL runs only, preserve LTR (digits) order
    for (level, range) in levels.into_iter().zip(ranges.into_iter()) {
        let slice = &text[range];
        if level.is_rtl() {
            // Only reverse if it's actually Arabic text (not digits/punctuation)
            let shaped = reshaper.reshape(slice);
            // Check if the slice contains Arabic letters vs just digits/symbols
            if slice.chars().any(|c| c >= '\u{0600}' && c <= '\u{06FF}') {
                // Contains Arabic - reverse after shaping
                let reversed: String = shaped.chars().rev().collect();
                out.push_str(&reversed);
            } else {
                // Just numbers/punctuation - don't reverse
                out.push_str(&shaped);
            }
        } else {
            out.push_str(slice);
        }
    }
    out
}

/// Render name (right-aligned) and price (left-aligned) in a space-between layout.
/// Returns (width, height, row_bytes) for the combined image.
/// Price gets priority - if name is too long, it will be truncated.
fn render_name_price_space_between(
    name: &str,
    price: &str,
    font_bytes: &[u8],
    font_px: f32,
    max_width: u32,
    bold: bool,
) -> (u32, u32, Vec<u8>) {
    let font = Font::try_from_bytes(font_bytes).expect("bad font");
    let reshaper = ArabicReshaper::new(ReshaperConfig::default());
    
    // Render price with currency (left side in final output, but right in Arabic)
    let price_text = format!("{} {}", price, "ج.م");
    let price_visual = bidi_then_shape(&price_text, &reshaper);
    
    // Render name (right side in final output, but left in Arabic)
    let name_visual = bidi_then_shape(name, &reshaper);
    
    let scale = Scale { x: font_px, y: font_px };
    let vm = font.v_metrics(scale);
    let ascent = vm.ascent.ceil();
    let descent = vm.descent.floor();
    let line_h = (ascent - descent).ceil().max(30.0) as u32;
    
    // Measure price width (always full)
    let price_glyphs: Vec<_> = font.layout(&price_visual, scale, point(0.0, ascent)).collect();
    let price_w = price_glyphs.iter().rev()
        .find_map(|g| g.pixel_bounding_box().map(|bb| bb.max.x as f32))
        .unwrap_or(0.0).ceil() as u32;
    
    // Measure name width
    let name_glyphs: Vec<_> = font.layout(&name_visual, scale, point(0.0, ascent)).collect();
    let name_w_full = name_glyphs.iter().rev()
        .find_map(|g| g.pixel_bounding_box().map(|bb| bb.max.x as f32))
        .unwrap_or(0.0).ceil() as u32;
    
    let min_gap = 10; // Minimum gap between name and price
    let left_padding = 5; // Left padding for price
    let available_for_name = max_width.saturating_sub(price_w + min_gap + left_padding);
    let name_w = name_w_full.min(available_for_name);
    
    let total_w = max_width;
    let mut img = ImageBuffer::from_pixel(total_w, line_h, Luma([255]));
    
    let passes: &[(i32,i32)] = if bold { &[(0,0),(1,0)] } else { &[(0,0)] };
    
    // Draw price on the left with 5px padding (x=5)
    for (dx, dy) in passes {
        for g in font.layout(&price_visual, scale, point(left_padding as f32 + *dx as f32, ascent + *dy as f32)) {
            if let Some(bb) = g.pixel_bounding_box() {
                g.draw(|x, y, v| {
                    if v > 0.5 {
                        let px = x + bb.min.x as u32;
                        let py = y + bb.min.y as u32;
                        if px < total_w && py < line_h { img.put_pixel(px, py, Luma([0])); }
                    }
                });
            }
        }
    }
    
    // Draw name on the right (x = total_w - name_w)
    let name_x = total_w - name_w;
    for &(_dx, _dy) in passes {
        for g in font.layout(&name_visual, scale, point(0.0, ascent)) {
            if let Some(bb) = g.pixel_bounding_box() {
                g.draw(|x, y, v| {
                    if v > 0.5 {
                        let px = x + bb.min.x as u32 + name_x;
                        let py = y + bb.min.y as u32;
                        if px < total_w && py < line_h { img.put_pixel(px, py, Luma([0])); }
                    }
                });
            }
        }
    }
    
    image_to_row_bytes(&img)
}

// ======== EPL2 helpers (binary GW + CRLF, optional invert) ========

fn epl_line(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.extend_from_slice(b"\r\n");
}

fn image_to_row_bytes(img: &ImageBuffer<Luma<u8>, Vec<u8>>) -> (u32,u32,Vec<u8>) {
    let (w,h) = (img.width(), img.height());
    let bpr = ((w + 7)/8) as usize;
    let mut out = vec![0u8; bpr*h as usize];

    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x,y).0[0] < 128 {
                let i = y as usize * bpr + (x as usize / 8);
                out[i] |= 1 << (7 - (x as usize % 8));
            }
        }
    }
    if INVERT_BITS { for b in &mut out { *b = !*b; } }
    (w,h,out)
}

fn gw_bytes(buf:&mut Vec<u8>, x:u32, y:u32, w:u32, h:u32, rows:&[u8]) {
    let bpr = ((w+7)/8) as usize;
    epl_line(buf, &format!("GW{},{},{},{}", x,y,bpr,h));
    buf.extend_from_slice(rows);  // RAW binary
    buf.extend_from_slice(b"\r\n");
}

fn center_x_for_ean13_single(label_w: u32, narrow: u32) -> u32 {
    let w = 95 * narrow; // EAN-13 total width (95 modules)
    (label_w - w) / 2
}

fn center_x_for_ean13_column(column_w: u32, narrow: u32) -> u32 {
    let w = 95 * narrow; // EAN-13 total width (95 modules)
    (column_w - w) / 2
}

// Ensure barcode is valid 12-digit EAN-13 (without check digit)
fn ensure_valid_ean13(barcode: &str) -> String {
    let digits: String = barcode.chars().filter(|c| c.is_ascii_digit()).collect();
    
    if digits.len() >= 12 {
        // Take first 12 digits (EPL2 will calculate check digit)
        digits[..12].to_string()
    } else if digits.len() == 13 {
        // If 13 digits provided, use first 12 (remove check digit)
        digits[..12].to_string()
    } else {
        // Pad with zeros to make 12 digits
        format!("{:0<12}", digits)
    }
}

// ======== Windows printer (optional, keep if you need send_raw_to_printer) ========

#[cfg(target_os = "windows")]
pub mod printer;

#[cfg(target_os = "windows")]
pub use printer::send_raw_to_printer;
