/// CPU context for cooperative task switching.
/// Only callee-saved registers need to be preserved across function calls.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub rsp: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rflags: u64,
}

impl Context {
    pub const fn empty() -> Self {
        Context {
            rsp: 0,
            rbp: 0,
            rbx: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rflags: 0x200, // interrupts enabled
        }
    }
}

/// Switch from `old` context to `new` context.
/// Saves callee-saved registers into `old`, restores from `new`.
///
/// # Safety
/// Both contexts must be valid and the `new` context's RSP must point to
/// a valid stack with a return address at the top.
#[naked]
pub unsafe extern "C" fn switch_context(old: *mut Context, new: *const Context) {
    core::arch::asm!(
        // Save callee-saved registers into old context
        "mov [rdi + 0x00], rsp",
        "mov [rdi + 0x08], rbp",
        "mov [rdi + 0x10], rbx",
        "mov [rdi + 0x18], r12",
        "mov [rdi + 0x20], r13",
        "mov [rdi + 0x28], r14",
        "mov [rdi + 0x30], r15",
        "pushfq",
        "pop qword ptr [rdi + 0x38]",

        // Restore callee-saved registers from new context
        "push qword ptr [rsi + 0x38]",
        "popfq",
        "mov rsp, [rsi + 0x00]",
        "mov rbp, [rsi + 0x08]",
        "mov rbx, [rsi + 0x10]",
        "mov r12, [rsi + 0x18]",
        "mov r13, [rsi + 0x20]",
        "mov r14, [rsi + 0x28]",
        "mov r15, [rsi + 0x30]",

        "ret",
        options(noreturn)
    );
}
