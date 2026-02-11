#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

extern crate alloc;

pub mod kernel;
pub mod drivers;
pub mod fs;
pub mod shell;
pub mod gui;

/// Early init: GDT, IDT, PICs, timer, syscall. No heap required.
pub fn init() {
    serial_println!("[INIT] GDT...");
    kernel::gdt::init();
    serial_println!("[INIT] IDT...");
    kernel::idt::init_idt();
    serial_println!("[INIT] PICs...");
    unsafe { kernel::pic::PICS.lock().initialize() };
    serial_println!("[INIT] Timer...");
    kernel::timer::init_timer();
    serial_println!("[INIT] Syscall...");
    kernel::syscall::init();
    serial_println!("[INIT] Early init done");
}

/// Late init: requires heap. Enables interrupts and initializes drivers.
pub fn init_late() {
    serial_println!("[INIT] Enabling interrupts...");
    x86_64::instructions::interrupts::enable();
    serial_println!("[INIT] ATA...");
    drivers::ata::init();
    serial_println!("[INIT] All done");
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    serial_println!("[PANIC] {}", info);
    println!("{}", info);
    hlt_loop()
}
