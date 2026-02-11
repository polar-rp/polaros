#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub rsp: u64,
}

impl Context {
    pub const fn empty() -> Self {
        Context {
            rsp: 0,
        }
    }
}

/// Switch between two tasks by saving/restoring callee-saved registers and RSP.
///
/// This saves rbp, rbx, r12-r15 on the old stack, saves the old RSP,
/// loads the new RSP, restores callee-saved regs, and returns to wherever
/// the new task's stack says (via `ret`).
///
/// # Safety
/// Both context pointers must be valid. The new context's RSP must point
/// to a valid stack with the correct layout.
#[naked]
pub unsafe extern "C" fn switch_context(old: *mut Context, new: *const Context) {
    core::arch::asm!(
        // Save callee-saved registers on old stack
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",

        // Save old RSP
        "mov [rdi], rsp",

        // Load new RSP
        "mov rsp, [rsi]",

        // Restore callee-saved registers from new stack
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",

        "ret",
        options(noreturn)
    );
}
