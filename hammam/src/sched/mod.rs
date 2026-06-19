extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use crate::arch::x86_64::context::{switch_context, Context as ArchContext};
use crate::drivers::serial::SpinMutex;
use crate::sched::task::{Task, TaskState};

pub mod task;

const QUANTUM_TICKS: u8 = 10;

pub struct Scheduler {
    pub run_queue: BTreeMap<u64, Arc<Task>>,
    pub current: Option<Arc<Task>>,
    pub min_vruntime: u64,
    current_ticks: u8,
}

impl Scheduler {
    pub const fn new() -> Self {
        Self {
            run_queue: BTreeMap::new(),
            current: None,
            min_vruntime: 0,
            current_ticks: 0,
        }
    }

    fn weight(priority: i32) -> u64 {
        let priority = priority.max(1).min(1024) as u64;
        1024 / priority
    }

    fn insert_task_into_queue(&mut self, task: Arc<Task>) {
        let mut vruntime = self.min_vruntime;
        if let Some(current) = self.current.as_ref() {
            vruntime = vruntime.min(current.vruntime);
        }

        let mut key = vruntime;
        while self.run_queue.contains_key(&key) {
            key = key.saturating_add(1);
        }

        if key != task.vruntime {
            unsafe {
                let task_ptr = Arc::as_ptr(&task) as *mut Task;
                (*task_ptr).vruntime = key;
            }
        }

        self.run_queue.insert(key, task);
    }

    pub fn add_task(&mut self, task: Arc<Task>) {
        if self.current.is_none() {
            self.current = Some(task);
        } else {
            self.insert_task_into_queue(task);
        }
    }

    pub fn pick_next(&mut self) -> Option<Arc<Task>> {
        if self.run_queue.is_empty() {
            return None;
        }

        if let Some(current) = self.current.take() {
            if current.state != TaskState::Dead {
                self.insert_task_into_queue(current);
            }
        }

        let (&next_vruntime, next_task) = self.run_queue.iter().next()?;
        let next = next_task.clone();
        self.run_queue.remove(&next_vruntime);
        self.min_vruntime = next_vruntime;
        self.current = Some(next.clone());
        Some(next)
    }

    fn schedule_locked(&mut self) -> Option<(*mut ArchContext, *const ArchContext)> {
        let current = self.current.as_ref()?.clone();
        let next = self.pick_next()?;
        if Arc::ptr_eq(&current, &next) {
            return None;
        }

        unsafe {
            let current_ptr = Arc::as_ptr(&current) as *mut Task;
            let next_ptr = Arc::as_ptr(&next) as *const Task as *mut Task;
            (*current_ptr).state = TaskState::Ready;
            (*next_ptr).state = TaskState::Running;
            Some((
                &mut (*current_ptr).context as *mut ArchContext,
                &(*next_ptr).context as *const ArchContext,
            ))
        }
    }

    pub fn schedule(&mut self) -> Option<(*mut ArchContext, *const ArchContext)> {
        self.schedule_locked()
    }

    pub fn tick(&mut self) -> Option<(*mut ArchContext, *const ArchContext)> {
        let current = self.current.as_ref()?;
        unsafe {
            let current_ptr = Arc::as_ptr(current) as *mut Task;
            let increment = 1_000_000 / Self::weight(current.priority);
            (*current_ptr).vruntime = (*current_ptr).vruntime.saturating_add(increment);
        }

        self.current_ticks = self.current_ticks.saturating_add(1);
        if self.current_ticks >= QUANTUM_TICKS {
            self.current_ticks = 0;
            self.schedule_locked()
        } else {
            None
        }
    }
}

pub static SCHEDULER: SpinMutex<Scheduler> = SpinMutex::new(Scheduler::new());

pub fn schedule_now() {
    if let Some((from, to)) = {
        let mut scheduler = SCHEDULER.lock();
        scheduler.schedule()
    } {
        unsafe { switch_context(from, to) }
    }
}

pub fn tick_now() {
    if let Some((from, to)) = {
        let mut scheduler = SCHEDULER.lock();
        scheduler.tick()
    } {
        unsafe { switch_context(from, to) }
    }
}

pub fn yield_now() {
    schedule_now();
}

#[cfg(test)]
mod tests {
    extern crate alloc;

    use super::*;
    use alloc::sync::Arc;
    use crate::kprintln;
    use crate::sched::task::{AddressSpace, Mutex};

    static mut MAIN_CONTEXT: ArchContext = ArchContext::default();

    extern "C" fn task_a() -> ! {
        kprintln!("[sched] task A");
        yield_now();
        kprintln!("[sched] task A done");
        exit_current();
    }

    extern "C" fn task_b() -> ! {
        kprintln!("[sched] task B");
        yield_now();
        kprintln!("[sched] task B done");
        exit_current();
    }

    extern "C" fn task_c() -> ! {
        kprintln!("[sched] task C");
        yield_now();
        kprintln!("[sched] task C done");
        exit_current();
    }

    fn create_task(entry: extern "C" fn() -> !) -> Arc<Task> {
        let mut task = Arc::new(Task::new(TaskId(0), 1, Arc::new(Mutex::new(AddressSpace))));

        let stack_top = task.kernel_stack.top;
        let stack_ptr = unsafe { stack_top.sub(core::mem::size_of::<u64>()) } as *mut u64;
        unsafe { *stack_ptr = entry as u64 };
        unsafe {
            let task_ptr = Arc::as_ptr(&task) as *mut Task;
            (*task_ptr).context.rsp = stack_ptr as u64;
            (*task_ptr).state = TaskState::Ready;
        }

        task
    }

    fn exit_current() -> ! {
        let mut return_pair = None;
        let mut current_task_ptr: Option<*mut Task> = None;

        {
            let mut scheduler = SCHEDULER.lock();
            if let Some(current) = scheduler.current.as_ref() {
                unsafe {
                    let current_task = Arc::as_ptr(current) as *mut Task;
                    (*current_task).state = TaskState::Dead;
                    current_task_ptr = Some(current_task);
                }
            }
            return_pair = scheduler.schedule();
        }

        if let Some((from, to)) = return_pair {
            unsafe { switch_context(from, to) }
        }

        if let Some(current_task) = current_task_ptr {
            unsafe { switch_context(&mut (*current_task).context as *mut ArchContext, &MAIN_CONTEXT as *const ArchContext) }
        }

        loop {}
    }

    #[test]
    fn round_robin_kernel_threads() {
        let t1 = create_task(task_a);
        let t2 = create_task(task_b);
        let t3 = create_task(task_c);

        {
            let mut scheduler = SCHEDULER.lock();
            scheduler.add_task(t1);
            scheduler.add_task(t2);
            scheduler.add_task(t3);
        }

        unsafe {
            switch_context(&mut MAIN_CONTEXT as *mut ArchContext, &SCHEDULER.lock().current.as_ref().unwrap().context as *const ArchContext);
        }
    }
}
