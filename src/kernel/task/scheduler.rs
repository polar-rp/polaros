use alloc::vec::Vec;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::{Context, switch_context};

const TASK_STACK_SIZE: usize = 4096 * 4; // 16 KiB per task

static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Terminated,
}

pub struct Task {
    pub id: TaskId,
    pub state: TaskState,
    pub context: Context,
    pub name: &'static str,
    _stack: Option<Box<[u8]>>,
}

impl Task {
    /// Create a new task with its own stack that will execute `entry_fn`.
    pub fn new(name: &'static str, entry_fn: fn()) -> Self {
        let id = TaskId(NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed));

        // Allocate stack
        let stack = Box::new([0u8; TASK_STACK_SIZE]);
        let stack_top = stack.as_ptr() as u64 + TASK_STACK_SIZE as u64;

        // Set up the stack so that `ret` from switch_context jumps to task_entry_trampoline,
        // which calls entry_fn. We put a return address on the stack.
        // The stack must be 16-byte aligned before the call, and the return address
        // pushes 8 bytes, so we align to 16 then subtract 8 for the return addr.
        let aligned_top = stack_top & !0xF; // 16-byte align
        let rsp = aligned_top - 8; // space for return address

        // Write the entry trampoline address as the return address
        unsafe {
            let ret_addr_ptr = rsp as *mut u64;
            *ret_addr_ptr = task_entry_trampoline as u64;
        }

        let mut ctx = Context::empty();
        ctx.rsp = rsp;
        ctx.rbp = 0;
        // Store the actual entry function pointer in r12 so the trampoline can call it
        ctx.r12 = entry_fn as u64;

        Task {
            id,
            state: TaskState::Ready,
            context: ctx,
            name,
            _stack: Some(stack),
        }
    }

    /// Create a "virtual" task representing the currently running kernel thread (task 0).
    /// Its context will be filled in when we first switch away from it.
    pub fn kernel_task() -> Self {
        Task {
            id: TaskId(0),
            state: TaskState::Running,
            context: Context::empty(),
            name: "kernel/shell",
            _stack: None, // uses the kernel stack
        }
    }
}

/// Trampoline that reads the entry function from r12 and calls it.
/// When the function returns, it marks the task as terminated and yields.
fn task_entry_trampoline() {
    // r12 contains the entry function pointer, set by Task::new
    let entry_fn: fn();
    unsafe {
        core::arch::asm!("mov {}, r12", out(reg) entry_fn);
    }
    entry_fn();
    // Task is done - mark as terminated and yield forever
    exit_current_task();
}

pub struct Scheduler {
    tasks: Vec<Task>,
    current: usize,
}

impl Scheduler {
    pub fn new() -> Self {
        let mut sched = Scheduler {
            tasks: Vec::new(),
            current: 0,
        };
        // Task 0 is the kernel/shell task
        sched.tasks.push(Task::kernel_task());
        sched
    }

    pub fn spawn(&mut self, name: &'static str, entry_fn: fn()) -> TaskId {
        let task = Task::new(name, entry_fn);
        let id = task.id;
        self.tasks.push(task);
        id
    }

    pub fn schedule(&mut self) {
        if self.tasks.len() <= 1 {
            return;
        }

        let old_idx = self.current;

        // Find next ready task (round-robin)
        let mut next_idx = (old_idx + 1) % self.tasks.len();
        let start = next_idx;
        loop {
            if self.tasks[next_idx].state == TaskState::Ready
                || self.tasks[next_idx].state == TaskState::Running
            {
                break;
            }
            next_idx = (next_idx + 1) % self.tasks.len();
            if next_idx == start {
                // No other runnable task, stay on current
                return;
            }
        }

        if next_idx == old_idx {
            return;
        }

        // Update states
        if self.tasks[old_idx].state == TaskState::Running {
            self.tasks[old_idx].state = TaskState::Ready;
        }
        self.tasks[next_idx].state = TaskState::Running;
        self.current = next_idx;

        let old_ctx = &mut self.tasks[old_idx].context as *mut Context;
        let new_ctx = &self.tasks[next_idx].context as *const Context;

        unsafe {
            switch_context(old_ctx, new_ctx);
        }
    }

    pub fn terminate_current(&mut self) {
        self.tasks[self.current].state = TaskState::Terminated;
    }

    pub fn kill_task(&mut self, id: TaskId) -> bool {
        for task in &mut self.tasks {
            if task.id == id && task.state != TaskState::Terminated {
                task.state = TaskState::Terminated;
                return true;
            }
        }
        false
    }

    pub fn task_list(&self) -> &[Task] {
        &self.tasks
    }

    pub fn cleanup_terminated(&mut self) {
        // Don't remove task 0 (kernel) or the current task
        self.tasks.retain(|t| {
            t.id == TaskId(0) || t.state != TaskState::Terminated
        });
        // Fix current index after cleanup
        for (i, task) in self.tasks.iter().enumerate() {
            if task.state == TaskState::Running {
                self.current = i;
                break;
            }
        }
    }
}

use spin::Mutex;

lazy_static::lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

/// Yield the CPU to the next ready task (cooperative).
pub fn yield_now() {
    // Disable interrupts during scheduling to prevent deadlocks
    x86_64::instructions::interrupts::without_interrupts(|| {
        SCHEDULER.lock().schedule();
    });
}

/// Mark the current task as terminated and yield.
pub fn exit_current_task() -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let mut sched = SCHEDULER.lock();
        sched.terminate_current();
        sched.schedule();
    });
    // Should never reach here, but just in case
    loop {
        x86_64::instructions::hlt();
    }
}

/// Spawn a new task.
pub fn spawn(name: &'static str, entry_fn: fn()) -> TaskId {
    x86_64::instructions::interrupts::without_interrupts(|| {
        SCHEDULER.lock().spawn(name, entry_fn)
    })
}
