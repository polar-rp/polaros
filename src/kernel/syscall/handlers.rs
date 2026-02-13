use alloc::string::String;
use alloc::vec::Vec;
use crate::kernel::task;
use crate::fs::{FS, FileSystem};

/// Syscall numbers
pub const SYS_EXIT: u64 = 0;
pub const SYS_WRITE: u64 = 1;
pub const SYS_YIELD: u64 = 2;
pub const SYS_GETPID: u64 = 3;
pub const SYS_OPEN: u64 = 4;
pub const SYS_READ: u64 = 5;
pub const SYS_CLOSE: u64 = 6;
pub const SYS_STAT: u64 = 7;

// Error codes (u64::MAX for backward compat, distinct constants for clarity)
const EBADF: u64 = u64::MAX;
const ENOENT: u64 = u64::MAX;
const EINVAL: u64 = u64::MAX;
const EMFILE: u64 = u64::MAX;

/// Per-task file descriptor table.
/// fd 0 = stdin (not really usable yet), fd 1 = stdout, fd 2 = stderr.
/// fd 3+ = opened files.
const MAX_FDS: usize = 16;
const RESERVED_FDS: usize = 3;

struct OpenFile {
    path: Vec<String>,
    name: String,
    offset: usize,
}

static mut FD_TABLE: [Option<OpenFile>; MAX_FDS] = {
    // Can't use array init with non-Copy types, use a const block
    const NONE: Option<OpenFile> = None;
    [NONE; MAX_FDS]
};

/// Read a user-space string from a pointer and length.
fn read_user_str(ptr: u64, len: u64) -> Option<&'static str> {
    let slice = unsafe { core::slice::from_raw_parts(ptr as *const u8, len as usize) };
    core::str::from_utf8(slice).ok()
}

fn alloc_fd() -> Option<usize> {
    unsafe {
        for i in RESERVED_FDS..MAX_FDS {
            if FD_TABLE[i].is_none() {
                return Some(i);
            }
        }
    }
    None
}

/// Main syscall dispatcher. Called from assembly entry point.
/// Returns value in RAX.
#[no_mangle]
pub extern "C" fn syscall_dispatch(nr: u64, arg0: u64, arg1: u64, arg2: u64) -> u64 {
    match nr {
        SYS_EXIT => sys_exit(arg0),
        SYS_WRITE => sys_write(arg0, arg1, arg2),
        SYS_YIELD => sys_yield(),
        SYS_GETPID => sys_getpid(),
        SYS_OPEN => sys_open(arg0, arg1),
        SYS_READ => sys_read(arg0, arg1, arg2),
        SYS_CLOSE => sys_close(arg0),
        SYS_STAT => sys_stat(arg0, arg1),
        _ => EINVAL
    }
}

/// sys_exit(code) - terminate current task
fn sys_exit(_code: u64) -> u64 {
    // Clean up all open FDs for this task
    unsafe {
        for i in RESERVED_FDS..MAX_FDS {
            FD_TABLE[i] = None;
        }
    }
    task::exit_current_task();
}

/// sys_write(fd, buf_ptr, len) - write to screen (fd=1 or fd=2 -> VGA)
fn sys_write(fd: u64, buf_ptr: u64, len: u64) -> u64 {
    if fd != 1 && fd != 2 {
        return EBADF;
    }

    let len = len as usize;
    let slice = unsafe { core::slice::from_raw_parts(buf_ptr as *const u8, len) };

    if let Ok(s) = core::str::from_utf8(slice) {
        crate::print!("{}", s);
        len as u64
    } else {
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

/// sys_open(path_ptr, path_len) -> fd or u64::MAX on error
/// Opens a file for reading. Path is relative to root.
fn sys_open(path_ptr: u64, path_len: u64) -> u64 {
    let path_str = match read_user_str(path_ptr, path_len) {
        Some(s) => s,
        None => return EINVAL,
    };

    let (dir_path, filename) = parse_file_path(path_str);

    {
        let fs = FS.lock();
        if !fs.exists(&dir_path, &filename) {
            return ENOENT;
        }
        if fs.is_dir(&dir_path, &filename) {
            return EINVAL;
        }
    }

    let fd = match alloc_fd() {
        Some(fd) => fd,
        None => return EMFILE,
    };

    unsafe {
        FD_TABLE[fd] = Some(OpenFile {
            path: dir_path,
            name: filename,
            offset: 0,
        });
    }

    fd as u64
}

/// sys_read(fd, buf_ptr, len) -> bytes_read or u64::MAX on error
fn sys_read(fd: u64, buf_ptr: u64, len: u64) -> u64 {
    let fd = fd as usize;
    if fd >= MAX_FDS {
        return EBADF;
    }

    let (read_bytes, new_offset) = unsafe {
        let file = match &FD_TABLE[fd] {
            Some(f) => f,
            None => return EBADF,
        };

        let fs = FS.lock();
        match fs.read(&file.path, &file.name) {
            Some(data) => {
                let remaining = if file.offset < data.len() {
                    &data[file.offset..]
                } else {
                    &[]
                };
                let to_read = remaining.len().min(len as usize);
                let dest = core::slice::from_raw_parts_mut(buf_ptr as *mut u8, to_read);
                dest.copy_from_slice(&remaining[..to_read]);
                (to_read, file.offset + to_read)
            }
            None => return ENOENT,
        }
    };

    // Update offset
    unsafe {
        if let Some(ref mut file) = FD_TABLE[fd] {
            file.offset = new_offset;
        }
    }

    read_bytes as u64
}

/// sys_close(fd) -> 0 on success, u64::MAX on error
fn sys_close(fd: u64) -> u64 {
    let fd = fd as usize;
    if fd < RESERVED_FDS || fd >= MAX_FDS {
        return EBADF;
    }
    unsafe {
        if FD_TABLE[fd].is_some() {
            FD_TABLE[fd] = None;
            0
        } else {
            EBADF
        }
    }
}

/// sys_stat(path_ptr, path_len) -> file size or u64::MAX on error
fn sys_stat(path_ptr: u64, path_len: u64) -> u64 {
    let path_str = match read_user_str(path_ptr, path_len) {
        Some(s) => s,
        None => return EINVAL,
    };

    let (dir_path, filename) = parse_file_path(path_str);

    let fs = FS.lock();
    match fs.read(&dir_path, &filename) {
        Some(data) => data.len() as u64,
        None => ENOENT,
    }
}

/// Parse a path like "/docs/info.txt" into (dir_components, filename)
fn parse_file_path(path: &str) -> (Vec<String>, String) {
    let path = path.trim_start_matches('/');
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        return (Vec::new(), String::new());
    }

    let filename = String::from(*parts.last().unwrap());
    let dir: Vec<String> = parts[..parts.len() - 1].iter().map(|s| String::from(*s)).collect();
    (dir, filename)
}
