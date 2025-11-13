use image::{ImageBuffer, Luma, DynamicImage};
use rusttype::{Font, Scale, point};
use ar_reshaper::{ArabicReshaper, ReshaperConfig};
use unicode_bidi::{BidiInfo, Level};

/// Return visually ordered string with Arabic runs reshaped, LTR runs unchanged.
/// This keeps numbers LTR and Arabic RTL, then we can render visually left→right.
fn bidi_then_shape(text: &str, reshaper: &ArabicReshaper) -> String {
    let info = BidiInfo::new(text, None);

    // Treat the paragraph as a single line
    let para = &info.paragraphs[0];
    let line = para.range.clone();

    // unicode-bidi 0.3 returns (levels, ranges)
    let (levels, ranges) = info.visual_runs(para, line);

    let mut out = String::new();
    for (level, range) in levels.into_iter().zip(ranges.into_iter()) {
        let slice = &text[range];
        if level.is_rtl() {
            out.push_str(&reshaper.reshape(slice)); // reshape only RTL
        } else {
            out.push_str(slice);                    // keep LTR (e.g., numbers)
        }
    }
    out
}

/// Render one Arabic line as a tight 1-bit image.
/// - We return the bitmap (tight width), so caller can right-align it.
pub fn render_arabic_line_tight_1bit(
    text: &str,
    font_bytes: &[u8],
    font_px: f32,       // e.g. 42.0
    pad_lr: u32,        // extra pixels to add around glyphs (e.g. 2..4)
) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    let font = Font::try_from_bytes(font_bytes).expect("bad font");
    let reshaper = ArabicReshaper::new(ReshaperConfig::default());

    // Correct visual order with proper shaping
    let visual = bidi_then_shape(text, &reshaper);

    let scale = Scale { x: font_px, y: font_px };
    let vm = font.v_metrics(scale);
    let ascent = vm.ascent.ceil();
    let descent = vm.descent.floor();
    let line_h = (ascent - descent).ceil().max(30.0) as u32;

    // Measure tight width
    let glyphs: Vec<_> = font.layout(&visual, scale, point(0.0, ascent)).collect();
    let text_w = glyphs.iter().rev()
        .find_map(|g| g.pixel_bounding_box().map(|bb| bb.max.x as f32))
        .unwrap_or(0.0)
        .ceil() as u32;

    let w = (text_w + pad_lr * 2).max(2);
    let mut img = ImageBuffer::from_pixel(w, line_h, Luma([255u8]));

    // Draw twice with 1-px offset for bold. Hard threshold to avoid gray.
    let baseline = ascent;
    for (dx, dy) in [(0i32,0i32), (1,0)].into_iter() {
        for g in font.layout(&visual, scale, point(pad_lr as f32 + dx as f32, baseline + dy as f32)) {
            if let Some(bb) = g.pixel_bounding_box() {
                g.draw(|x, y, v| {
                    if v > 0.65 {
                        let px = x + bb.min.x as u32;
                        let py = y + bb.min.y as u32;
                        if px < w && py < line_h {
                            img.put_pixel(px, py, Luma([0]));
                        }
                    }
                });
            }
        }
    }

    img
}

/// Rotate 90 degrees clockwise to compensate for driver-locked landscape orientation.
pub fn rotate90(img: &ImageBuffer<Luma<u8>, Vec<u8>>) -> ImageBuffer<Luma<u8>, Vec<u8>> {
    DynamicImage::ImageLuma8(img.clone()).rotate90().to_luma8()
}
