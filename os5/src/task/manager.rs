//! Implementation of [`TaskManager`]
//!
//! It is only used to manage processes and schedule process based on ready queue.
//! Other CPU process monitoring functions are in Processor.


use alloc::collections::VecDeque;
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use lazy_static::*;
use crate::task::current_task;

pub struct TaskManager {
    // ready_queue: VecDeque<Arc<TaskControlBlock>>,
    task_queue: VecDeque<Arc<TaskControlBlock>>,
}

// YOUR JOB: FIFO->Stride
/// A simple FIFO scheduler.
impl TaskManager {
    pub fn new() -> Self {
        Self {
            task_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        if task.pass() == 0 {
            self.task_queue.push_front(task);
        }else {
            self.task_queue.push_back(task);
        }
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        let task = self.task_queue.iter().enumerate().min_by_key(|(_id, task)| {
            let inner_ref = (*task).inner_exclusive_access();
            inner_ref.info.priority
        });
        let task_idx = task.unwrap().0;
        // info!("Task {:?} is fetched!", task_idx);
        self.task_queue.remove(task_idx)
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
