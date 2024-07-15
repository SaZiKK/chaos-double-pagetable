//! Types related to task management & Functions for completely changing TCB

use alloc::sync::{Arc, Weak};
use core::cell::RefMut;

use super::{id::TaskUserRes, kstack_alloc, KernelStack, ProcessControlBlock, TaskContext};
use crate::{
    config::{BIG_STRIDE, MAX_SYSCALL_NUM},
    fs::inode::Inode,
    mm::PhysPageNum,
    sync::UPSafeCell,
    trap::TrapContext,
};

/// Task control block structure
pub struct TaskControlBlock {
    /// immutable
    pub process: Weak<ProcessControlBlock>,
    /// Kernel stack corresponding to PID
    pub kstack: KernelStack,
    /// mutable
    inner: UPSafeCell<TaskControlBlockInner>,
}

impl TaskControlBlock {
    /// Get the mutable reference of the inner TCB
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    /// Get the address of app's page table
    pub fn get_user_token(&self) -> usize {
        let process = self.process.upgrade().unwrap();
        let inner = process.inner_exclusive_access();
        inner.memory_set.token()
    }
}

pub struct TaskControlBlockInner {
    pub res: Option<TaskUserRes>,
    /// The physical page number of the frame where the trap context is placed
    pub trap_cx_ppn: PhysPageNum,
    /// Save task context
    pub task_cx: TaskContext,
    /// Maintain the execution status of the current process
    pub task_status: TaskStatus,
    /// It is set when active exit or execution error occurs
    pub exit_code: Option<i32>,
    /// syscall times of tasks
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// the time task was first run
    pub first_time: Option<usize>,
    /// priority
    pub priority: usize,
    /// stride
    pub stride: usize,
    /// pass
    pub pass: usize,
    ///
    pub clear_child_tid: usize,
    /// working directory
    pub work_dir: Arc<Inode>,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }

    #[allow(unused)]
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }

    pub fn gettid(&self) -> usize {
        self.res.as_ref().unwrap().tid
    }
}

impl TaskControlBlock {
    /// Create a new task
    pub fn new(process: Arc<ProcessControlBlock>, ustack_top: usize, alloc_user_res: bool) -> Self {
        let res = TaskUserRes::new(Arc::clone(&process), ustack_top, alloc_user_res);
        let trap_cx_ppn = res.trap_cx_ppn();
        let kstack = kstack_alloc();
        let kstack_top = kstack.get_top();
        let process_inner = process.inner_exclusive_access();
        let work_dir = Arc::clone(&process_inner.work_dir);
        drop(process_inner);
        Self {
            process: Arc::downgrade(&process),
            kstack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    res: Some(res),
                    trap_cx_ppn,
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    exit_code: None,
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    first_time: None,
                    priority: 16,
                    stride: 0,
                    pass: BIG_STRIDE / 16,
                    work_dir,
                    clear_child_tid: 0,
                })
            },
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
/// The execution status of the current process
pub enum TaskStatus {
    /// ready to run
    Ready,
    /// running
    Running,
    /// blocked
    Blocked,
}
