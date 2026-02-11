use super::framebuffer::Framebuffer;

// 8x12 arrow cursor bitmap (1 = draw, 0 = transparent)
const CURSOR_WIDTH: i16 = 8;
const CURSOR_HEIGHT: i16 = 12;

#[rustfmt::skip]
const CURSOR_BITMAP: [u8; 12] = [
    0b1000_0000,
    0b1100_0000,
    0b1110_0000,
    0b1111_0000,
    0b1111_1000,
    0b1111_1100,
    0b1111_1110,
    0b1111_0000,
    0b1101_1000,
    0b1000_1100,
    0b0000_0110,
    0b0000_0011,
];

#[rustfmt::skip]
const CURSOR_OUTLINE: [u8; 12] = [
    0b1000_0000,
    0b1100_0000,
    0b1010_0000,
    0b1001_0000,
    0b1000_1000,
    0b1000_0100,
    0b1000_0010,
    0b1001_0000,
    0b1101_1000,
    0b1000_1100,
    0b0000_0110,
    0b0000_0011,
];

pub struct Cursor {
    pub x: i16,
    pub y: i16,
    pub visible: bool,
}

impl Cursor {
    pub fn new(x: i16, y: i16) -> Self {
        Cursor { x, y, visible: true }
    }

    pub fn render(&self, fb: &mut Framebuffer) {
        if !self.visible {
            return;
        }
        for row in 0..CURSOR_HEIGHT {
            let fill_bits = CURSOR_BITMAP[row as usize];
            let outline_bits = CURSOR_OUTLINE[row as usize];
            for col in 0..CURSOR_WIDTH {
                let mask = 0x80u8 >> col;
                let px = self.x + col;
                let py = self.y + row;
                if outline_bits & mask != 0 {
                    // Outline: draw black
                    fb.set_pixel(px, py, 15); // BLACK
                } else if fill_bits & mask != 0 {
                    // Fill: draw white (TEXT_BRIGHT)
                    fb.set_pixel(px, py, 8);
                }
            }
        }
    }
}
