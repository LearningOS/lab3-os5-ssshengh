use core::cmp::Ordering;
use crate::config::{BIG_PRIORITY, MAX_SYSCALL_NUM};
use crate::timer::{get_time_us};

#[derive(Clone, Copy)]
pub struct Info {
    /// Syscall times called by current task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Current task's start time
    pub start_time: usize,
    /// Priority of current task
    pub priority: Priority,
}

impl Info {
    pub(crate) fn set_priority(&mut self, p0: usize) {
        self.priority.priority = p0;
    }
}

impl Info {
    pub fn record_start_time(&mut self) {
        let time = get_time_us()/1000;
        self.start_time = time;
    }

    pub fn record_syscall(&mut self, syscall_id: usize) {
        self.syscall_times[syscall_id] += 1;
    }

    pub fn during_time(&self) -> usize {
        get_time_us()/1000 - self.start_time
    }
}

impl Default for Info {
    fn default() -> Self {
        Self {
            syscall_times: [0; MAX_SYSCALL_NUM],
            start_time: 0,
            priority: Default::default(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Priority {
    pub(crate) pass: u64,
    pub priority: usize,
    round: u32,
}

impl Default for Priority {
    fn default() -> Self {
        Self {
            pass: 0,
            priority: 16,
            round: 0,
        }
    }
}

impl Priority {
    pub fn update_pass(&mut self) {
        let res = self.pass
            .overflowing_add((BIG_PRIORITY / self.priority) as u64);
        if res.1 {
            self.round += 1;
        }
        self.pass = res.0;
    }
}

impl PartialEq for Priority {
    fn eq(&self, other: &Self) -> bool {
        self.round == other.round && self.pass == other.pass
    }
}

impl PartialOrd for Priority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.round != other.round{
            return self.round.partial_cmp(&other.round);
        }
        self.pass.partial_cmp(&other.pass)
    }
}

impl Eq for Priority {}

impl Ord for Priority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap()
    }
}




// pub fn record_task_start_time() {
//     let task = current_task().unwrap();
//     let mut task_cx = task.inner_exclusive_access();
//
//     task_cx.info.record_start_time();
// }
//
//
// pub fn get_running_time() -> usize {
//     let task = current_task().unwrap();
//     let task_cx = task.inner_exclusive_access();
//
//     let now = get_time_us();
//     now - task_cx.info.start_time
// }
//
// pub fn get_syscall_nums() -> [u32; MAX_SYSCALL_NUM] {
//     let task = current_task().unwrap();
//     let task_cx = task.inner_exclusive_access();
//     task_cx.info.syscall_times.clone()
// }
//
// pub fn record_syscall(id: u32) {
//     let task = current_task().unwrap();
//     let mut task_cx = task.inner_exclusive_access();
//     task_cx.info.record_syscall(id);
// }
