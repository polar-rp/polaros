use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::structures::idt::InterruptStackFrame;
use crate::kernel::pic::{InterruptIndex, PICS};
use crate::kernel::task::context::switch_context;

pub const TIMER_HZ: u32 = 100;

const PIT_BASE_FREQUENCY: u32 = 1_193_182;
const PIT_CMD_PORT: u16 = 0x43;
const PIT_DATA_PORT: u16 = 0x40;
const PIT_CMD_SQUARE_WAVE: u8 = 0x36;

static TICKS: AtomicU64 = AtomicU64::new(0);

pub fn init_timer() {
    use x86_64::instructions::port::Port;
    let divisor: u16 = (PIT_BASE_FREQUENCY / TIMER_HZ) as u16;
    unsafe {
        let mut cmd = Port::<u8>::new(PIT_CMD_PORT);
        let mut data = Port::<u8>::new(PIT_DATA_PORT);
        cmd.write(PIT_CMD_SQUARE_WAVE);
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

/// Naked interrupt handler for Timer.
/// Saves context, calls rust handler, schedules, restores context.
#[naked]
pub extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    unsafe {
        core::arch::asm!(
            // 1. Save all GPRs (except RSP which is already saved by CPU)
            "push rax",
            "push rbx",
            "push rcx",
            "push rdx",
            "push rsi",
            "push rdi",
            "push rbp",
            "push r8",
            "push r9",
            "push r10",
            "push r11",
            "push r12",
            "push r13",
            "push r14",
            "push r15",

            // 2. Call Rust handler to update ticks and ACK PIC
            "call rust_timer_handler",

            // 3. Call Scheduler to switch tasks (preemptive)
            "call scheduler_schedule",

            // 4. Restore all GPRs
            "pop r15",
            "pop r14",
            "pop r13",
            "pop r12",
            "pop r11",
            "pop r10",
            "pop r9",
            "pop r8",
            "pop rbp",
            "pop rdi",
            "pop rsi",
            "pop rdx",
            "pop rcx",
            "pop rbx",
            "pop rax",

            // 5. Return from interrupt
            "iretq",
            options(noreturn)
        );
    }
}

#[no_mangle]
pub extern "C" fn rust_timer_handler() {
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

/// Preemptive scheduler entry point called from the timer interrupt handler.
/// Uses try_lock to avoid deadlocks — if the scheduler is already locked
/// (e.g., during yield_now or spawn), we simply skip this tick.
#[no_mangle]
pub extern "C" fn scheduler_schedule() {
    use crate::kernel::task::SCHEDULER;

    let switch = {
        if let Some(mut sched) = SCHEDULER.try_lock() {
            sched.schedule_preempt()
        } else {
            return; // scheduler busy, skip this tick
        }
    }; // lock dropped here — BEFORE context switch

    if let Some((old_ctx, new_ctx)) = switch {
        unsafe { switch_context(old_ctx, new_ctx); }
    }
}
