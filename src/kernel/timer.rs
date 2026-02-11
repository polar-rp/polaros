use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::InterruptStackFrame;

use crate::kernel::pic::{InterruptIndex, PICS};

pub const TIMER_HZ: u32 = 100;
static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init_timer() {
    use x86_64::instructions::port::Port;
    let divisor: u16 = (1_193_182u32 / TIMER_HZ) as u16;
    unsafe {
        let mut cmd = Port::<u8>::new(0x43);
        let mut data = Port::<u8>::new(0x40);
        cmd.write(0x36);
        data.write((divisor & 0xFF) as u8);
        data.write((divisor >> 8) as u8);
    }
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

struct FmtBuf {
    buf: [u8; 40],
    pos: usize,
}

impl FmtBuf {
    fn new() -> Self {
        FmtBuf { buf: [0; 40], pos: 0 }
    }
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos]).unwrap_or("")
    }
}

impl core::fmt::Write for FmtBuf {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for &byte in s.as_bytes() {
            if self.pos < self.buf.len() {
                self.buf[self.pos] = byte;
                self.pos += 1;
            }
        }
        Ok(())
    }
}

pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let ticks = TICKS.fetch_add(1, Ordering::Relaxed) + 1;

    // Only update VGA text status bar when NOT in GUI mode
    if !crate::gui::GUI_MODE_ACTIVE.load(Ordering::Relaxed) {
        if ticks % TIMER_HZ as u64 == 0 {
            use core::fmt::Write;
            let secs = ticks / TIMER_HZ as u64;
            let h = secs / 3600;
            let m = (secs % 3600) / 60;
            let s = secs % 60;
            let mut buf = FmtBuf::new();
            let _ = write!(buf, " {}h {:02}m {:02}s ", h, m, s);
            crate::drivers::vga::update_status_bar(" PolarOs v0.1.0", buf.as_str());
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}
