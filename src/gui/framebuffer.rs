use alloc::boxed::Box;

pub const SCREEN_WIDTH: u16 = 320;
pub const SCREEN_HEIGHT: u16 = 200;
const FB_SIZE: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize; // 64000

const VGA_FRAMEBUFFER: *mut u8 = 0xA0000 as *mut u8;

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
