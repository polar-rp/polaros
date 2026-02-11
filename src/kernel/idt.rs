use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::kernel::gdt;
use crate::kernel::pic::InterruptIndex;
use crate::kernel::timer;
use crate::println;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer::timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler);
        idt[InterruptIndex::Mouse.as_usize()].set_handler_fn(mouse_interrupt_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    crate::hlt_loop();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    use crate::kernel::pic::PICS;

    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    if crate::gui::GUI_MODE_ACTIVE.load(core::sync::atomic::Ordering::Relaxed) {
        crate::drivers::keyboard::add_scancode_gui(scancode);
    } else {
        crate::drivers::keyboard::add_scancode(scancode);
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    use crate::kernel::pic::PICS;

    let mut port = Port::new(0x60);
    let byte: u8 = unsafe { port.read() };

    crate::drivers::mouse::handle_byte(byte);

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Mouse.as_u8());
    }
}
