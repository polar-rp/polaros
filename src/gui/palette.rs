use x86_64::instructions::port::Port;

// Core UI colors (indices 0-15)
pub const BG_DARK: u8 = 0;
pub const BG_BASE: u8 = 1;
pub const BG_SURFACE: u8 = 2;
pub const BG_ELEVATED: u8 = 3;
pub const BG_HIGHLIGHT: u8 = 4;
pub const BORDER: u8 = 5;
pub const TEXT_MUTED: u8 = 6;
pub const TEXT_PRIMARY: u8 = 7;
pub const TEXT_BRIGHT: u8 = 8;
pub const ACCENT_BLUE: u8 = 9;
pub const ACCENT_HOVER: u8 = 10;
pub const SUCCESS: u8 = 11;
pub const WARNING: u8 = 12;
pub const ERROR: u8 = 13;
pub const ACCENT_PURPLE: u8 = 14;
pub const BLACK: u8 = 15;

// VGA DAC uses 6-bit color (0-63), so we convert 8-bit (0-255) by shifting right 2
fn rgb(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    (r >> 2, g >> 2, b >> 2)
}

fn set_dac_color(index: u8, r6: u8, g6: u8, b6: u8) {
    unsafe {
        let mut addr_port = Port::<u8>::new(0x3C8);
        let mut data_port = Port::<u8>::new(0x3C9);
        addr_port.write(index);
        data_port.write(r6);
        data_port.write(g6);
        data_port.write(b6);
    }
}

fn set_color(index: u8, r: u8, g: u8, b: u8) {
    let (r6, g6, b6) = rgb(r, g, b);
    set_dac_color(index, r6, g6, b6);
}

fn lerp(a: u8, b: u8, t: u8, steps: u8) -> u8 {
    let a = a as u16;
    let b = b as u16;
    let t = t as u16;
    let steps = steps as u16;
    (a + (b - a) * t / steps) as u8
}

fn set_ramp(start_index: u8, r0: u8, g0: u8, b0: u8, r1: u8, g1: u8, b1: u8, count: u8) {
    for i in 0..count {
        let r = lerp(r0, r1, i, count - 1);
        let g = lerp(g0, g1, i, count - 1);
        let b = lerp(b0, b1, i, count - 1);
        set_color(start_index + i, r, g, b);
    }
}

pub fn load_palette() {
    // Core UI colors (0-15)
    set_color(0,  0x14, 0x14, 0x20); // BG_DARK
    set_color(1,  0x20, 0x20, 0x30); // BG_BASE
    set_color(2,  0x2C, 0x2C, 0x40); // BG_SURFACE
    set_color(3,  0x38, 0x38, 0x50); // BG_ELEVATED
    set_color(4,  0x48, 0x48, 0x60); // BG_HIGHLIGHT
    set_color(5,  0x64, 0x64, 0x78); // BORDER
    set_color(6,  0x80, 0x80, 0x90); // TEXT_MUTED
    set_color(7,  0xC0, 0xC0, 0xC8); // TEXT_PRIMARY
    set_color(8,  0xF0, 0xF0, 0xFC); // TEXT_BRIGHT
    set_color(9,  0x38, 0x58, 0xC8); // ACCENT_BLUE
    set_color(10, 0x50, 0x78, 0xE8); // ACCENT_HOVER
    set_color(11, 0x20, 0xA0, 0x80); // SUCCESS
    set_color(12, 0xDC, 0x8C, 0x20); // WARNING
    set_color(13, 0xC8, 0x30, 0x30); // ERROR
    set_color(14, 0x70, 0x38, 0xC0); // ACCENT_PURPLE
    set_color(15, 0x00, 0x00, 0x00); // BLACK

    // Grayscale ramp (16-31)
    set_ramp(16, 0x08, 0x08, 0x08, 0xF8, 0xF8, 0xF8, 16);

    // Blue ramp (32-47)
    set_ramp(32, 0x08, 0x08, 0x30, 0x60, 0x90, 0xF0, 16);

    // Purple ramp (48-63)
    set_ramp(48, 0x20, 0x08, 0x30, 0xB0, 0x60, 0xF0, 16);

    // Teal ramp (64-79)
    set_ramp(64, 0x08, 0x20, 0x20, 0x40, 0xE0, 0xD0, 16);

    // Green ramp (80-95)
    set_ramp(80, 0x08, 0x20, 0x08, 0x50, 0xE0, 0x50, 16);

    // Orange ramp (96-111)
    set_ramp(96, 0x30, 0x18, 0x08, 0xF0, 0xA0, 0x30, 16);

    // Red ramp (112-127)
    set_ramp(112, 0x30, 0x08, 0x08, 0xF0, 0x40, 0x40, 16);

    // Reserved (128-255) - set to black
    for i in 128u8..=255 {
        set_color(i, 0, 0, 0);
    }
}
