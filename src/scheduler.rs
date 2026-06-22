use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::interrupts as cpu_irq;

use crate::{println, serial_println};

const MAX_TASKS: usize = 8;

pub type TaskFn = fn(&mut TaskContext);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Finished,
}

pub struct TaskContext {
    pub task_id: usize,
    pub tick: u64,
    yielded: bool,
    finished: bool,
}

impl TaskContext {
    pub fn yield_now(&mut self) {
        self.yielded = true;
    }

    pub fn finish(&mut self) {
        self.finished = true;
    }
}

#[allow(dead_code)]
struct Task {
    name: &'static str,
    func: TaskFn,
    state: TaskState,
    runs: u64,
}

impl Task {
    const fn new(name: &'static str, func: TaskFn) -> Self {
        Self { name, func, state: TaskState::Ready, runs: 0 }
    }
}

pub struct Scheduler {
    tasks: [Option<Task>; MAX_TASKS],
    current: usize,
    tick: u64,
}

impl Scheduler {
    const fn new() -> Self {
        Self { tasks: [None, None, None, None, None, None, None, None], current: 0, tick: 0 }
    }

    pub fn add_task(&mut self, name: &'static str, func: TaskFn) -> usize {
        for (idx, slot) in self.tasks.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(Task::new(name, func));
                return idx;
            }
        }
        panic!("scheduler task table full");
    }

    fn next_runnable_from(&self, start: usize) -> Option<usize> {
        for offset in 0..MAX_TASKS {
            let idx = (start + offset) % MAX_TASKS;
            if matches!(self.tasks[idx].as_ref().map(|t| t.state), Some(TaskState::Ready)) {
                return Some(idx);
            }
        }
        None
    }

    /// Prepare to run the next task: mark it Running and return (idx, fn, ctx).
    /// Returns None if no task is ready.
    fn prepare_next(&mut self) -> Option<(usize, TaskFn, TaskContext)> {
        let idx = self.next_runnable_from(self.current)?;
        let task = self.tasks[idx].as_mut().expect("task missing");
        task.state = TaskState::Running;
        task.runs += 1;
        let ctx = TaskContext { task_id: idx, tick: self.tick, yielded: false, finished: false };
        Some((idx, task.func, ctx))
    }

    /// Update task state after its function has returned.
    fn finish_task(&mut self, idx: usize, ctx: &TaskContext) {
        let task = self.tasks[idx].as_mut().expect("task missing");
        task.state = if ctx.finished { TaskState::Finished } else { TaskState::Ready };
        self.current = if ctx.yielded { (idx + 1) % MAX_TASKS } else { idx };
    }

    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }
}

lazy_static! {
    static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

static SCHEDULER_TICKED: AtomicBool = AtomicBool::new(false);
static BOOT_TICK_COUNT: AtomicU64 = AtomicU64::new(0);

pub fn init() {
    let mut sched = SCHEDULER.lock();
    sched.add_task("idle", idle_task);
    sched.add_task("worker-a", worker_task);
    sched.add_task("worker-b", logger_task);

    serial_println!("[OK] scheduler: {} task slots populated", 3);
    println!("[OK] scheduler ready");
}

pub fn on_timer_tick() {
    BOOT_TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    // Release so the updated count is visible when logger_task reads it.
    SCHEDULER_TICKED.store(true, Ordering::Release);
    SCHEDULER.lock().tick();
}

pub fn run() -> ! {
    loop {
        // Phase 1: acquire lock, pick next task, release lock.
        let prepared = SCHEDULER.lock().prepare_next();

        match prepared {
            None => {
                // No runnable task — atomically enable interrupts and halt.
                // This avoids missing a wakeup that arrives between STI and HLT.
                cpu_irq::enable_and_hlt();
            }
            Some((idx, func, mut ctx)) => {
                // Phase 2: call task function with NO lock held so timer ISR
                // can update the scheduler without deadlocking.
                func(&mut ctx);

                // Phase 3: re-acquire lock to commit the result.
                SCHEDULER.lock().finish_task(idx, &ctx);
            }
        }
    }
}

fn idle_task(ctx: &mut TaskContext) {
    if ctx.tick % 32 == 0 {
        serial_println!("[sched] idle task on tick {}", ctx.tick);
    }
    ctx.yield_now();
}

fn worker_task(ctx: &mut TaskContext) {
    if ctx.tick % 16 == 0 {
        serial_println!("[sched] worker-a tick {}", ctx.tick);
        println!("[sched] worker-a tick {}", ctx.tick);
    }
    ctx.yield_now();
}

fn logger_task(ctx: &mut TaskContext) {
    if SCHEDULER_TICKED.swap(false, Ordering::Acquire) {
        let total = BOOT_TICK_COUNT.load(Ordering::Relaxed);
        serial_println!("[sched] timer tick {}", total);
    }
    if ctx.tick > 128 {
        serial_println!("[sched] logger task finished");
        ctx.finish();
        return;
    }
    ctx.yield_now();
}
