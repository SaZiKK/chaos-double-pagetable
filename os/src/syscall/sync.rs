use riscv::register::sstatus;
use crate::{
    board::CLOCK_FREQ,
    task::{current_task, suspend_current_and_run_next},
    timer::{get_time, NSEC_PER_SEC},
};
/// sleep syscall
pub fn sys_sleep(time_req: *const u64, time_remain: *mut u64) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().pid.0,
        current_task().unwrap().tid
    );
    #[inline]
    fn is_end(end_time: usize) -> bool {
        let current_time = get_time();
        current_time >= end_time
    }
    unsafe {
        sstatus::set_sum();
        let sec = *time_req;
        let nano_sec = *time_req.add(1);
        sstatus::clear_sum();
        let end_time =
            get_time() + sec as usize * CLOCK_FREQ + nano_sec as usize * CLOCK_FREQ / NSEC_PER_SEC;

        loop {
            if is_end(end_time) {
                break;
            } else {
                suspend_current_and_run_next()
            }
        }

        sstatus::set_sum();
        if time_remain as usize != 0 {
            *time_remain = 0;
            *time_remain.add(1) = 0;
        }
        sstatus::clear_sum();
    }
    0
}


// /// mutex create syscall
// pub fn sys_mutex_create(blocking: bool) -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_mutex_create",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let mutex: Option<Arc<dyn MutexSupport>> = if !blocking {
//         Some(Arc::new(SpinNoIrqLock::new()))
//     } else {
//         Some(Arc::new(SpinNoIrqLock::new()))
//     };
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     if let Some(id) = process_inner
//         .mutex_list
//         .iter()
//         .enumerate()
//         .find(|(_, item)| item.is_none())
//         .map(|(id, _)| id)
//     {
//         process_inner.mutex_list[id] = mutex;
//         process_inner.available[id] = 1;
//         for task in &mut process_inner.allocation {
//             task[id] = 0;
//         }
//         for task in &mut process_inner.need {
//             task[id] = 0;
//         }
//         id as isize
//     } else {
//         process_inner.mutex_list.push(mutex);
//         process_inner.available.push(1);
//         for task in &mut process_inner.allocation {
//             task.push(0);
//         }
//         for task in &mut process_inner.need {
//             task.push(0);
//         }
//         process_inner.mutex_list.len() as isize - 1
//     }
// }

// /// mutex lock syscall
// pub fn sys_mutex_lock(mutex_id: usize) -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_mutex_lock",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
//     let tid = current_task().unwrap().inner_exclusive_access(file!(), line!()).res.as_ref().unwrap().tid;
//     process_inner.need[tid][mutex_id] += 1;
//     let deadlock_detect = process_inner.deadlock_detect;
//     drop(process_inner);
//     drop(process);
//     if deadlock_detect && detect_deadlock() {
//         return -0xdead;
//     }
//     mutex.lock();
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     process_inner.available[mutex_id] -= 1;
//     let tid = current_task().unwrap().inner_exclusive_access(file!(), line!()).res.as_ref().unwrap().tid;
//     process_inner.allocation[tid][mutex_id] += 1;
//     process_inner.need[tid][mutex_id] -= 1;
//     0
// }

// /// mutex unlock syscall
// pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_mutex_unlock",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let process_inner = process.inner_exclusive_access(file!(), line!());
//     let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
//     drop(process_inner);
//     drop(process);
//     mutex.unlock();
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     process_inner.available[mutex_id] += 1;
//     let tid = current_task().unwrap().inner_exclusive_access(file!(), line!()).res.as_ref().unwrap().tid;
//     process_inner.allocation[tid][mutex_id] -= 1;
//     0
// }

// /// semaphore create syscall
// pub fn sys_semaphore_create(res_count: usize) -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_semaphore_create",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     let id = if let Some(id) = process_inner
//         .semaphore_list
//         .iter()
//         .enumerate()
//         .find(|(_, item)| item.is_none())
//         .map(|(id, _)| id)
//     {
//         process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
//         process_inner.available[id] = res_count as u32;
//         for task in &mut process_inner.allocation {
//             task[id] = 0;
//         }
//         for task in &mut process_inner.need {
//             task[id] = 0;
//         }
//         id
//     } else {
//         process_inner
//             .semaphore_list
//             .push(Some(Arc::new(Semaphore::new(res_count))));
//         process_inner.available.push(res_count as u32);
//         for task in &mut process_inner.allocation {
//             task.push(0);
//         }
//         for task in &mut process_inner.need {
//             task.push(0);
//         }
//         process_inner.semaphore_list.len() - 1
//     };
//     id as isize
// }

// /// semaphore up syscall
// pub fn sys_semaphore_up(sem_id: usize) -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_semaphore_up",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let process_inner = process.inner_exclusive_access(file!(), line!());
//     let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
//     drop(process_inner);
//     drop(process);
//     sem.up();
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     process_inner.available[sem_id] += 1;
//     let tid = current_task().unwrap().inner_exclusive_access(file!(), line!()).res.as_ref().unwrap().tid;
//     process_inner.allocation[tid][sem_id] -= 1;
//     0
// }

// /// semaphore down syscall
// pub fn sys_semaphore_down(sem_id: usize) -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_semaphore_down",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
//     let tid = current_task().unwrap().inner_exclusive_access(file!(), line!()).res.as_ref().unwrap().tid;
//     process_inner.need[tid][sem_id] += 1;
//     let deadlock_detect = process_inner.deadlock_detect;
//     drop(process_inner);
//     drop(process);
//     if deadlock_detect && detect_deadlock() {
//         return -0xdead;
//     }
//     sem.down();
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     process_inner.available[sem_id] -= 1;
//     let tid = current_task().unwrap().inner_exclusive_access(file!(), line!()).res.as_ref().unwrap().tid;
//     process_inner.allocation[tid][sem_id] += 1;
//     process_inner.need[tid][sem_id] -= 1;
//     0
// }

// /// condvar create syscall
// pub fn sys_condvar_create() -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_condvar_create",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     let id = if let Some(id) = process_inner
//         .condvar_list
//         .iter()
//         .enumerate()
//         .find(|(_, item)| item.is_none())
//         .map(|(id, _)| id)
//     {
//         process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
//         id
//     } else {
//         process_inner
//             .condvar_list
//             .push(Some(Arc::new(Condvar::new())));
//         process_inner.condvar_list.len() - 1
//     };
//     id as isize
// }

// /// condvar signal syscall
// pub fn sys_condvar_signal(condvar_id: usize) -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_condvar_signal",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let process_inner = process.inner_exclusive_access(file!(), line!());
//     let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
//     drop(process_inner);
//     condvar.signal();
//     0
// }

// /// condvar wait syscall
// pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
//     trace!(
//         "kernel:pid[{}] tid[{}] sys_condvar_wait",
//         current_task().unwrap().process.upgrade().unwrap().getpid(),
//         current_task()
//             .unwrap()
//             .inner_exclusive_access(file!(), line!())
//             .res
//             .as_ref()
//             .unwrap()
//             .tid
//     );
//     let process = current_process();
//     let process_inner = process.inner_exclusive_access(file!(), line!());
//     let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
//     let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
//     drop(process_inner);
//     condvar.wait(mutex);
//     0
// }

///// enable deadlock detection syscall
// //
// pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
//     trace!("kernel: sys_enable_deadlock_detect");
//     if enabled != 0 && enabled != 1 {
//         return -1;
//     }
//     let process = current_process();
//     let mut process_inner = process.inner_exclusive_access(file!(), line!());
//     process_inner.deadlock_detect = enabled == 1;
//     0
// }
