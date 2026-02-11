pub mod handlers;
pub mod userprogs;

use x86_64::registers::model_specific::{Star, LStar, SFMask, Efer, EferFlags};
use x86_64::registers::rflags::RFlags;
use x86_64::VirtAddr;

use crate::kernel::gdt;

/// Initialize the syscall/sysret mechanism by programming MSRs.
pub fn init() {
    let selectors = gdt::selectors();

    unsafe {
        // Enable SCE (System Call Extensions) in EFER
        Efer::update(|flags| {
            *flags |= EferFlags::SYSTEM_CALL_EXTENSIONS;
        });

        // STAR: bits 47:32 = kernel CS selector, bits 63:48 = sysret base
        // For sysret (64-bit): user CS = STAR[63:48] + 16, user SS = STAR[63:48] + 8
        // For syscall: kernel CS = STAR[47:32], kernel SS = STAR[47:32] + 8
        // sysret base must be: user_data_selector - 8 (so +8 = user_data, +16 = user_code)
        let sysret_base = selectors.user_data_selector.0 - 8;
        Star::write_raw(sysret_base, selectors.code_selector.0);

        // LSTAR: target RIP for syscall instruction
        LStar::write(VirtAddr::new(syscall_entry_asm as u64));

        // SFMASK: clear IF (interrupt flag) on syscall entry for safety
        SFMask::write(RFlags::INTERRUPT_FLAG);
    }
}

/// The assembly entry point for the `syscall` instruction.
/// On entry:
///   RCX = user RIP (return address)
///   R11 = user RFLAGS
///   RAX = syscall number
///   RDI = arg0, RSI = arg1, RDX = arg2
#[naked]
unsafe extern "C" fn syscall_entry_asm() {
    core::arch::asm!(
        // We're now in ring 0 but still on the user stack.
        // For safety we should switch to the kernel stack,
        // but for simplicity in this minimal implementation
        // we use the user stack (which is mapped in kernel space too).

        // Save user registers
        "push rcx",   // user RIP
        "push r11",   // user RFLAGS

        // Call the Rust dispatcher: syscall_dispatch(nr, arg0, arg1, arg2)
        // RAX=nr is already in rdi position after we shuffle:
        // Current: RAX=nr, RDI=arg0, RSI=arg1, RDX=arg2
        // Need:    RDI=nr, RSI=arg0, RDX=arg1, RCX=arg2
        "mov rcx, rdx",  // arg2
        "mov rdx, rsi",  // arg1
        "mov rsi, rdi",  // arg0
        "mov rdi, rax",  // nr

        "call {dispatch}",

        // RAX now has the return value

        // Restore user registers
        "pop r11",    // user RFLAGS
        "pop rcx",    // user RIP

        "sysretq",
        dispatch = sym handlers::syscall_dispatch,
        options(noreturn)
    );
}
