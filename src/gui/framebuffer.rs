use alloc::boxed::Box;
use x86_64::instructions::port::Port;

pub const SCREEN_WIDTH: u16 = 320;
pub const SCREEN_HEIGHT: u16 = 200;
const FB_SIZE: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize; // 64000

const VGA_FRAMEBUFFER: *mut u8 = 0xA0000 as *mut u8;

/// Standard VGA Mode 13h register values (320x200x256 linear)
/// Reference: FreeVGA, OSDev wiki
const MODE_13H_MISC: u8 = 0x63;

const MODE_13H_SEQ: [u8; 5] = [
    0x03, // Reset
    0x01, // Clocking Mode (8-dot)
    0x0F, // Map Mask (all planes)
    0x00, // Character Map Select
    0x0E, // Sequencer Memory Mode (chain-4)
];

const MODE_13H_CRTC: [u8; 25] = [
    0x5F, // Horizontal Total
    0x4F, // Horizontal Display End
    0x50, // Start Horizontal Blanking
    0x82, // End Horizontal Blanking
    0x54, // Start Horizontal Retrace
    0x80, // End Horizontal Retrace
    0xBF, // Vertical Total
    0x1F, // Overflow
    0x00, // Preset Row Scan
    0x41, // Maximum Scan Line
    0x00, // Cursor Start
    0x00, // Cursor End
    0x00, // Start Address High
    0x00, // Start Address Low
    0x00, // Cursor Location High
    0x00, // Cursor Location Low
    0x9C, // Start Vertical Retrace
    0x0E, // End Vertical Retrace (also unlocks CRTC)
    0x8F, // Vertical Display End
    0x28, // Offset (logical width / 8 = 320/8 = 40 = 0x28)
    0x40, // Underline Location
    0x96, // Start Vertical Blanking
    0xB9, // End Vertical Blanking
    0xA3, // Mode Control
    0xFF, // Line Compare
];

const MODE_13H_GC: [u8; 9] = [
    0x00, // Set/Reset
    0x00, // Enable Set/Reset
    0x00, // Color Compare
    0x00, // Data Rotate
    0x00, // Read Map Select
    0x40, // Graphics Mode (256-color)
    0x05, // Miscellaneous Graphics
    0x0F, // Color Don't Care
    0xFF, // Bit Mask
];

const MODE_13H_AC: [u8; 21] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
    0x41, // Attribute Mode Control
    0x00, // Overscan Color
    0x0F, // Color Plane Enable
    0x00, // Horizontal Pixel Panning
    0x00, // Color Select
];

/// Switch VGA hardware to Mode 13h (320x200, 256 colors, linear framebuffer)
/// Must be called before any framebuffer writes.
pub fn set_mode_13h() {
    unsafe {
        // Write Miscellaneous Output Register
        Port::<u8>::new(0x3C2).write(MODE_13H_MISC);

        // Sequencer registers
        for (i, &val) in MODE_13H_SEQ.iter().enumerate() {
            Port::<u8>::new(0x3C4).write(i as u8);
            Port::<u8>::new(0x3C5).write(val);
        }

        // Unlock CRTC (clear protect bit in register 0x11)
        Port::<u8>::new(0x3D4).write(0x11);
        let val = Port::<u8>::new(0x3D5).read();
        Port::<u8>::new(0x3D4).write(0x11);
        Port::<u8>::new(0x3D5).write(val & 0x7F);

        // CRTC registers
        for (i, &val) in MODE_13H_CRTC.iter().enumerate() {
            Port::<u8>::new(0x3D4).write(i as u8);
            Port::<u8>::new(0x3D5).write(val);
        }

        // Graphics Controller registers
        for (i, &val) in MODE_13H_GC.iter().enumerate() {
            Port::<u8>::new(0x3CE).write(i as u8);
            Port::<u8>::new(0x3CF).write(val);
        }

        // Attribute Controller registers
        // Reading 0x3DA resets the AC flip-flop to index mode
        let _ = Port::<u8>::new(0x3DA).read();
        for (i, &val) in MODE_13H_AC.iter().enumerate() {
            Port::<u8>::new(0x3C0).write(i as u8);
            Port::<u8>::new(0x3C0).write(val);
        }
        // Re-enable video output (set bit 5)
        Port::<u8>::new(0x3C0).write(0x20);

        // Clear framebuffer to black
        core::ptr::write_bytes(VGA_FRAMEBUFFER, 0, FB_SIZE);
    }
}

pub struct Framebuffer {
    buffer: Box<[u8; FB_SIZE]>,
}

impl Framebuffer {
    pub fn new() -> Self {
        Framebuffer {
            buffer: Box::new([0u8; FB_SIZE]),
        }
    }

    #[inline]
    pub fn set_pixel(&mut self, x: i16, y: i16, color: u8) {
        if x >= 0 && x < SCREEN_WIDTH as i16 && y >= 0 && y < SCREEN_HEIGHT as i16 {
            self.buffer[y as usize * SCREEN_WIDTH as usize + x as usize] = color;
        }
    }

    #[inline]
    pub fn get_pixel(&self, x: i16, y: i16) -> u8 {
        if x >= 0 && x < SCREEN_WIDTH as i16 && y >= 0 && y < SCREEN_HEIGHT as i16 {
            self.buffer[y as usize * SCREEN_WIDTH as usize + x as usize]
        } else {
            0
        }
    }

    #[inline]
    pub fn xor_pixel(&mut self, x: i16, y: i16) {
        if x >= 0 && x < SCREEN_WIDTH as i16 && y >= 0 && y < SCREEN_HEIGHT as i16 {
            let idx = y as usize * SCREEN_WIDTH as usize + x as usize;
            self.buffer[idx] ^= 0xFF;
        }
    }

    pub fn clear(&mut self, color: u8) {
        for byte in self.buffer.iter_mut() {
            *byte = color;
        }
    }

    pub fn present(&self) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.buffer.as_ptr(),
                VGA_FRAMEBUFFER,
                FB_SIZE,
            );
        }
    }
}
