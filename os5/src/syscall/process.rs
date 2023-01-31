//! Process management syscalls

use crate::loader::get_app_data_by_name;
use crate::mm::{MapPermission, translated_refmut, translated_str, VirtAddr};
use crate::task::{add_task, current_task, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskControlBlock, TaskStatus};
use crate::timer::{get_time_us};
use alloc::sync::Arc;
use crate::config::MAX_SYSCALL_NUM;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}


#[derive(Clone, Copy)]
pub struct TaskInfo {
    pub status: TaskStatus,
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    pub time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    debug!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    current_task().unwrap().pid.0 as isize
}

/// Syscall Fork which returns 0 for child process and child_pid for parent process
pub fn sys_fork() -> isize {
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

/// Syscall Exec which accepts the elf path
pub fn sys_exec(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        task.record_start();
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    let task = current_task().unwrap();
    // find a child process

    // ---- access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB lock exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after removing from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child TCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB lock automatically
}

// YOUR JOB: 引入虚地址后重写 sys_get_time
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let current_task = current_task().unwrap();
    let task_cx = current_task.inner_exclusive_access();
    let ts_user_space = translated_refmut(task_cx.memory_set.token(), ts);

    let us = get_time_us();
    (*ts_user_space).sec = us/1_000_000;
    (*ts_user_space).usec = us%1_000_000;

    0
}

// YOUR JOB: 引入虚地址后重写 sys_task_info
pub fn sys_task_info(ti: *mut TaskInfo) -> isize {
    let current_task = current_task().unwrap();
    let current_cx = current_task.inner_exclusive_access();
    let task_info = translated_refmut(current_cx.memory_set.token(), ti);

    (*task_info).status = current_cx.task_status;
    let sys_num = current_cx.info.syscall_times.clone();
    (*task_info).syscall_times = sys_num;
    (*task_info).time = current_cx.info.during_time();
    0
}

// YOUR JOB: 实现sys_set_priority，为任务添加优先级
pub fn sys_set_priority(prio: isize) -> isize {
    if prio < 2 {
        return -1;
    }
    let current_task = current_task().unwrap();
    current_task.set_priority(prio as usize);
    prio
}

// YOUR JOB: 扩展内核以实现 sys_mmap 和 sys_munmap
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    let perm = change_port_to_permission(port);
    if perm.is_none() {
        return -1;
    }

    let start_va: VirtAddr = start.into();
    if !start_va.aligned() {
        return -1;
    }
    let end_va: VirtAddr = (start + len).into();

    let task = current_task().unwrap();
    let mut task_cx = task.inner_exclusive_access();
    if let Some(()) = task_cx.memory_set.insert_framed_area_check(start_va, end_va, perm.unwrap()){
        0
    }else {
        -1
    }
}

pub fn sys_munmap(start: usize, len: usize) -> isize {
    let start_va: VirtAddr = start.into();
    if !start_va.aligned() {
        return -1;
    }
    let end_va: VirtAddr = (start + len).into();
    let task = current_task().unwrap();
    let mut task_cx = task.inner_exclusive_access();
    if let Some(()) = task_cx.memory_set.move_frame_area_check(start_va, end_va) {
        0
    }else {
        -1
    }
}

//
// YOUR JOB: 实现 sys_spawn 系统调用
// ALERT: 注意在实现 SPAWN 时不需要复制父进程地址空间，SPAWN != FORK + EXEC 
pub fn sys_spawn(path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, path);
    let data = get_app_data_by_name(path.as_str());
    // Check the file name
    if data.is_none() {
        return -1;
    }

    let current_task = current_task().unwrap();
    // New task
    let data = data.unwrap();
    let new_task = Arc::new(TaskControlBlock::new(data));
    let pid = new_task.getpid();
    // Fix the parent-child relationship
    let mut task_cx_child = new_task.inner_exclusive_access();
    task_cx_child.parent = Some(Arc::downgrade(&current_task));
    drop(task_cx_child);
    let mut task_cx_parent = current_task.inner_exclusive_access();
    task_cx_parent.children.push(new_task.clone());

    add_task(new_task);


    pid as isize
}

fn change_port_to_permission(port: usize) -> Option<MapPermission> {
    let user_permission = MapPermission::U;
    let (read, write, execute) = (MapPermission::R, MapPermission::W, MapPermission::X);
    let permission = match port {
        1 => Some(read | user_permission),
        2 => Some(write | user_permission),
        4 => Some(execute | user_permission),
        3 => Some(read | write | user_permission),
        5 => Some(read | execute | user_permission),
        6 => Some(write | execute | user_permission),
        7 => Some(write | execute | read | user_permission),
        _ => None
    };
    permission
}