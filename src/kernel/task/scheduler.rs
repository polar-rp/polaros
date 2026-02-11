use alloc::vec::Vec;
use alloc::boxed::Box;
use core::sync::atomic::{AtomicU64, Ordering};

use super::context::{Context, switch_context};

const TASK_STACK_SIZE: usize = 4096 * 4; // 16 KiB per task
const DEFAULT_QUANTUM: u32 = 10; // 10 ticks = 100ms at 100Hz

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
    pub quantum_remaining: u32,
    _stack: Option<Box<[u8]>>,
}

impl Task {
    /// Create a new task with its own stack that will execute `entry_fn`.
    ///
    /// Stack layout (low address → high address):
    ///   [r15=0, r14=0, r13=0, r12=entry_fn, rbx=0, rbp=0, ret_addr=trampoline]
    ///
    /// This matches what `switch_context` expects: it pops r15..rbp then `ret`.
    pub fn new(name: &'static str, entry_fn: fn()) -> Self {
        let id = TaskId(NEXT_TASK_ID.fetch_add(1, Ordering::Relaxed));

        // Allocate stack
        let stack = Box::new([0u8; TASK_STACK_SIZE]);
        let stack_top = stack.as_ptr() as u64 + TASK_STACK_SIZE as u64;

        // Build the initial stack frame for switch_context
        // switch_context does: pop r15, r14, r13, r12, rbx, rbp, ret
        // So we push in reverse order: ret_addr, rbp, rbx, r12, r13, r14, r15
        let mut rsp = stack_top;

        // Return address — where `ret` in switch_context will jump to
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = task_entry_trampoline as u64; }

        // rbp = 0
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        // rbx = 0
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        // r12 = entry_fn pointer (trampoline reads this)
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = entry_fn as u64; }

        // r13 = 0
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        // r14 = 0
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        // r15 = 0 (switch_context pops this first)
        rsp -= 8;
        unsafe { *(rsp as *mut u64) = 0; }

        let mut ctx = Context::empty();
        ctx.rsp = rsp;

        Task {
            id,
            state: TaskState::Ready,
            context: ctx,
            name,
            quantum_remaining: DEFAULT_QUANTUM,
            _stack: Some(stack),
        }
    }

    /// Create a "virtual" task representing the currently running kernel thread (task 0).
    pub fn kernel_task() -> Self {
        Task {
            id: TaskId(0),
            state: TaskState::Running,
            context: Context::empty(),
            name: "kernel",
            quantum_remaining: DEFAULT_QUANTUM,
            _stack: None, // uses the kernel stack
        }
    }
}

/// Trampoline that reads the entry function from r12 and calls it.
/// When the function returns, it marks the task as terminated and yields.
extern "C" fn task_entry_trampoline() {
    // Re-enable interrupts — we may have been switched to from a timer handler
    // where interrupts were disabled.
    x86_64::instructions::interrupts::enable();

    // R12 holds the function pointer (set up by Task::new)
    let entry_fn: fn();
    unsafe {
        core::arch::asm!("mov {}, r12", out(reg) entry_fn);
    }
    entry_fn();
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

    /// Decide whether to switch tasks (preemptive path).
    /// Decrements the quantum; if expired, finds the next ready task.
    /// Returns context pointers for the switch, or None if no switch needed.
    pub fn schedule_preempt(&mut self) -> Option<(*mut Context, *const Context)> {
        if self.tasks.len() <= 1 {
            return None;
        }

        // Decrement quantum for current task
        if self.tasks[self.current].quantum_remaining > 0 {
            self.tasks[self.current].quantum_remaining -= 1;
            if self.tasks[self.current].quantum_remaining > 0 {
                return None; // still has time left
            }
        }

        self.find_and_switch()
    }

    /// Cooperative yield: always try to switch to next task.
    pub fn schedule_yield(&mut self) -> Option<(*mut Context, *const Context)> {
        if self.tasks.len() <= 1 {
            return None;
        }
        self.find_and_switch()
    }

    /// Find the next runnable task and prepare context pointers for switching.
    fn find_and_switch(&mut self) -> Option<(*mut Context, *const Context)> {
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
                // No other runnable task — reset quantum and stay
                self.tasks[old_idx].quantum_remaining = DEFAULT_QUANTUM;
                return None;
            }
        }

        if next_idx == old_idx {
            // Only one runnable task — reset quantum and stay
            self.tasks[old_idx].quantum_remaining = DEFAULT_QUANTUM;
            return None;
        }

        // Update states
        if self.tasks[old_idx].state == TaskState::Running {
            self.tasks[old_idx].state = TaskState::Ready;
        }
        self.tasks[next_idx].state = TaskState::Running;
        self.tasks[next_idx].quantum_remaining = DEFAULT_QUANTUM;
        self.current = next_idx;

        let old_ctx = &mut self.tasks[old_idx].context as *mut Context;
        let new_ctx = &self.tasks[next_idx].context as *const Context;

        Some((old_ctx, new_ctx))
    }

    /// Legacy cooperative schedule (calls switch_context internally).
    /// Used only by yield_now for backward compat.
    pub fn schedule(&mut self) {
        if let Some((old_ctx, new_ctx)) = self.schedule_yield() {
            unsafe {
                switch_context(old_ctx, new_ctx);
            }
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

impl Scheduler {
    pub unsafe fn force_unlock() {
        SCHEDULER.force_unlock();
    }
}

/// Yield the CPU to the next ready task (cooperative).
pub fn yield_now() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let switch = {
            let mut sched = SCHEDULER.lock();
            sched.schedule_yield()
        }; // lock dropped here

        if let Some((old_ctx, new_ctx)) = switch {
            unsafe { switch_context(old_ctx, new_ctx); }
        }
    });
}

/// Mark the current task as terminated and yield.
pub fn exit_current_task() -> ! {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let switch = {
            let mut sched = SCHEDULER.lock();
            sched.terminate_current();
            sched.schedule_yield()
        }; // lock dropped

        if let Some((old_ctx, new_ctx)) = switch {
            unsafe { switch_context(old_ctx, new_ctx); }
        }
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
