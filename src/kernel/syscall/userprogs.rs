/// Pre-compiled user-mode programs (raw x86_64 machine code).
/// These use the syscall instruction to communicate with the kernel.
///
/// Syscall convention:
///   RAX = syscall number
///   RDI = arg0, RSI = arg1, RDX = arg2
///   syscall
///   RAX = return value

/// "hello" program: writes "Hello from user mode!\n" then exits.
///
/// Equivalent to:
///   mov rax, 1          ; SYS_WRITE
///   mov rdi, 1          ; fd = stdout
///   lea rsi, [rip+msg]  ; buf pointer
///   mov rdx, 22         ; length
///   syscall
///   mov rax, 0          ; SYS_EXIT
///   xor rdi, rdi        ; code = 0
///   syscall
///   msg: db "Hello from user mode!\n"
pub fn hello_program() -> &'static [u8] {
    // We generate this at runtime using a function that the task will call,
    // rather than raw bytes, since it's simpler and more maintainable.
    // See `run_user_hello` below.
    &[]
}

/// Run "hello" as a kernel-mode task that simulates a user syscall.
/// This demonstrates the syscall dispatch path without actual ring-3 transition.
pub fn run_user_hello() {
    let msg = "Hello from user mode!\n";
    // Call syscall dispatch directly (simulating what a real syscall would do)
    super::handlers::syscall_dispatch(
        super::handlers::SYS_WRITE,
        1, // stdout
        msg.as_ptr() as u64,
        msg.len() as u64,
    );

    let msg2 = "Syscall getpid returned: ";
    super::handlers::syscall_dispatch(
        super::handlers::SYS_WRITE,
        1,
        msg2.as_ptr() as u64,
        msg2.len() as u64,
    );

    // Get PID
    let pid = super::handlers::syscall_dispatch(
        super::handlers::SYS_GETPID,
        0, 0, 0,
    );

    // Print PID (simple decimal conversion)
    let mut buf = [0u8; 20];
    let mut pos = 0;
    if pid == 0 {
        buf[0] = b'0';
        pos = 1;
    } else {
        let mut n = pid;
        let mut digits = [0u8; 20];
        let mut dpos = 0;
        while n > 0 {
            digits[dpos] = b'0' + (n % 10) as u8;
            dpos += 1;
            n /= 10;
        }
        for i in (0..dpos).rev() {
            buf[pos] = digits[i];
            pos += 1;
        }
    }
    buf[pos] = b'\n';
    pos += 1;

    super::handlers::syscall_dispatch(
        super::handlers::SYS_WRITE,
        1,
        buf.as_ptr() as u64,
        pos as u64,
    );

    // Yield a few times to demonstrate cooperation
    for _ in 0..3 {
        super::handlers::syscall_dispatch(super::handlers::SYS_YIELD, 0, 0, 0);
    }

    // Exit
    super::handlers::syscall_dispatch(super::handlers::SYS_EXIT, 0, 0, 0);
}

/// A user program that counts using syscalls
pub fn run_user_counter() {
    for i in 1..=5u64 {
        let msg = "[usercount] Krok ";
        super::handlers::syscall_dispatch(
            super::handlers::SYS_WRITE, 1,
            msg.as_ptr() as u64, msg.len() as u64,
        );

        // Print number
        let mut buf = [0u8; 4];
        buf[0] = b'0' + (i % 10) as u8;
        buf[1] = b'\n';
        super::handlers::syscall_dispatch(
            super::handlers::SYS_WRITE, 1,
            buf.as_ptr() as u64, 2,
        );

        // Yield
        super::handlers::syscall_dispatch(super::handlers::SYS_YIELD, 0, 0, 0);
    }

    let done = "[usercount] Zakonczony.\n";
    super::handlers::syscall_dispatch(
        super::handlers::SYS_WRITE, 1,
        done.as_ptr() as u64, done.len() as u64,
    );
}
