use super::framebuffer::Framebuffer;
use font8x8::legacy::BASIC_LEGACY;

pub const CHAR_WIDTH: u16 = 8;
pub const CHAR_HEIGHT: u16 = 8;

pub fn draw_char(fb: &mut Framebuffer, x: i16, y: i16, ch: char, color: u8) {
    let idx = ch as usize;
    if idx >= BASIC_LEGACY.len() {
        return;
    }
    let glyph = BASIC_LEGACY[idx];
    for (row, &bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            if bits & (1 << col) != 0 {
                fb.set_pixel(x + col as i16, y + row as i16, color);
            }
        }
    }
}

pub fn draw_char_bg(fb: &mut Framebuffer, x: i16, y: i16, ch: char, fg: u8, bg: u8) {
    let idx = ch as usize;
    if idx >= BASIC_LEGACY.len() {
        return;
    }
    let glyph = BASIC_LEGACY[idx];
    for (row, &bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            let color = if bits & (1 << col) != 0 { fg } else { bg };
            fb.set_pixel(x + col as i16, y + row as i16, color);
        }
    }
}

pub fn draw_text(fb: &mut Framebuffer, x: i16, y: i16, text: &str, color: u8) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char(fb, cx, y, ch, color);
        cx += CHAR_WIDTH as i16;
    }
}

pub fn draw_text_bg(fb: &mut Framebuffer, x: i16, y: i16, text: &str, fg: u8, bg: u8) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char_bg(fb, cx, y, ch, fg, bg);
        cx += CHAR_WIDTH as i16;
    }
}

pub fn text_width(text: &str) -> u16 {
    text.len() as u16 * CHAR_WIDTH
}
