use crate::kernel::task;

/// Syscall numbers
pub const SYS_EXIT: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_YIELD: u64 = 2;
pub const SYS_GETPID: u64 = 3;

/// Main syscall dispatcher. Called from assembly entry point.
/// Returns value in RAX.
#[no_mangle]
pub extern "C" fn syscall_dispatch(nr: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    match nr {
        SYS_EXIT => sys_exit(arg0),
        SYS_WRITE => sys_write(arg0, arg1, arg2),
        SYS_YIELD => sys_yield(),
        SYS_GETPID => sys_getpid(),
        _ => {
            // Unknown syscall
            u64::MAX
        }
    }
}

/// sys_exit(code) - terminate current task
fn sys_exit(_code: u64) -> u64 {
    task::exit_current_task();
}

/// sys_write(fd, buf_ptr, len) - write to screen (fd=1 -> VGA)
fn sys_write(fd: u64, buf_ptr: u64, len: u64) -> u64 {
    if fd != 1 {
        return u64::MAX; // only stdout supported
    }

    let len = len as usize;
    // Safety: we trust that the user program has a valid buffer.
    // In a real OS we'd validate the pointer is in user space.
    let slice = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, len) };

    if let Ok(s) = core::str::from_utf8(slice) {
        crate::print!("{}", s);
        len as u64
    } else {
        // Write raw bytes
        for &byte in slice {
            if byte >= 0x20 && byte <= 0x7e || byte == b'\n' {
                crate::print!("{}", byte as char);
            }
        }
        len as u64
    }
}

/// sys_yield() - cooperative yield
fn sys_yield() -> u64 {
    task::yield_now();
    0
}

/// sys_getpid() - return current task ID
fn sys_getpid() -> u64 {
    let sched = task::SCHEDULER.lock();
    let tasks = sched.task_list();
    for t in tasks {
        if t.state == task::TaskState::Running {
            return t.id.0;
        }
    }
    0
}
