#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]

extern crate alloc;

pub mod kernel;
pub mod drivers;
pub mod fs;
pub mod shell;
pub mod gui;

pub fn init() {
    kernel::gdt::init();
    kernel::idt::init_idt();
    unsafe { kernel::pic::PICS.lock().initialize() };
    kernel::timer::init_timer();
    kernel::syscall::init();
    x86_64::instructions::interrupts::enable();
    drivers::ata::init();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    hlt_loop()
}
