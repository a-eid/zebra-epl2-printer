// Label and printer tuning constants
pub const LABEL_W: u32 = 440;
pub const LABEL_H: u32 = 320;

pub const PAD_RIGHT: u32 = 10;
pub const FONT_PX: f32 = 44.0;   // larger
pub const DARKNESS: u8 = 5;      // reduce banding
pub const SPEED: u8 = 3;

pub const NARROW: u32 = 2;       // EAN-13 module width (2–3)
pub const HEIGHT: u32 = 50;      // bar height

pub const FORCE_LANDSCAPE: bool = true; // rotate content in code if driver prints landscape
pub const INVERT_BITS: bool = true;     // flip GW bits → black text on white
