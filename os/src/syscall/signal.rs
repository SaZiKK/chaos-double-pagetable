use riscv::register::{sscratch, sstatus};

use crate::{
    mm::{translated_ref, translated_refmut},
    syscall::errno::{EAGAIN, EPERM, SUCCESS},
    task::{
        current_task,
        sigaction::SignalAction,
        signal::{SigInfo, MAX_SIG, SIG_BLOCK, SIG_SETMASK, SIG_UNBLOCK},
        suspend_current_and_run_next,
        SignalFlags,
    },
    timer::TimeSpec,
};

/// 一个系统调用，用于获取和设置信号的屏蔽位。通过 `sigprocmask`，进程可以方便的屏蔽某些信号。
///
/// 参数：
/// + `how`: 指明将采取何种逻辑修改信号屏蔽位。大致包括：屏蔽 `set` 中指明的所有信号，将 `set` 中指明的所有信号解除屏蔽或者直接使用 `set` 作为屏蔽码。具体可见 [`SigProcMaskHow`]。
/// + `set`: 用于指明将要修改的信号屏蔽位。具体可见 [`SimpleBitSet`]。当该值为 null 时，将不修改信号的屏蔽位。
/// + `oldset`: 用于获取当前对信号的屏蔽位。具体可见 [`SimpleBitSet`]。当该值为 null 时，将不保存信号的旧屏蔽位。
/// + `_sig_set_size`: 用于指示 `set` 和 `oldset` 所指向的信号屏蔽位的长度，目前在 Alien 中未使用。
///
/// 函数正常执行后，返回 0。
///
/// Reference: [sigprocmask](https://www.man7.org/linux/man-pages/man2/sigprocmask.2.html)
pub fn sys_sigprocmask(
    how: usize, set: *mut usize, old_set: *mut usize, kernel_space: bool,
) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sigprocmask",
        current_task().unwrap().pid.0,
        current_task().unwrap().tid
    );
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());

    let mut mask = inner.signal_mask;

    if kernel_space {
        if old_set as usize != 0 {
            unsafe {
                sstatus::set_sum();
                *old_set = mask.bits();
                sstatus::clear_sum();
            }
        }
    } else {
        if old_set as usize != 0 {
            unsafe {
                sstatus::set_sum();
                *old_set = mask.bits();
                sstatus::clear_sum();
            }
        }
    }

    if set as usize != 0 {
        let mut new_set = 0;
        unsafe {
            sstatus::set_sum();
            new_set = *set;
            sstatus::clear_sum();
        }
        // tip!("[sys_sigprocmask] set = {:#b}, how = {}", set, how);
        let set_flags = SignalFlags::from_bits(new_set).unwrap();
        // if set_flags.contains(SignalFlags::SIGILL) {
        //     log!("[sys_sigprocmask] SignalFlags::SIGILL");
        // }
        match how {
            // SIG_BLOCK The set of blocked signals is the union of the current set and the set argument.
            SIG_BLOCK => mask |= set_flags,
            // SIG_UNBLOCK The signals in set are removed from the current set of blocked signals.
            SIG_UNBLOCK => mask &= !set_flags,
            // SIG_SETMASK The set of blocked signals is set to the argument set.
            SIG_SETMASK => mask = set_flags,
            _ => return EPERM,
        }
        inner.signal_mask = mask;
    }
    SUCCESS
}

/// 一个系统调用，用于获取或修改与指定信号相关联的处理动作。
///
/// 一个进程，对于每种信号，在不进行特殊设置的情况下，都有其默认的处理方式。有关信号的处理流程具体可见 [`signal_handler`] 与 [`SigActionDefault`]。
/// 用户可以通过 `sigaction` 获取或修改进程在接收到某信号时的处理动作。
///
/// 参数：
/// + `sig`: 指出要修改的处理动作所捕获的信号类型。有关详情可见 [`SignalNumber`]。
/// + `action`: 指定新的信号处理方式的指针。详情可见 [`SigAction`]。当该值为空指针时，`sigaction` 将不会修改信号的处理动作。
/// + `old_action`: 指出原信号处理方式要保存到的位置。详情可见 [`SigAction`]。当该值为空指针时，`sigaction` 将不会保存信号的原处理动作。
///
/// 函数执行成功后返回 0；若输入的 `sig` 是 `SIGSTOP`, `SIGKILL`, `ERR`中的一个时，将导致函数返回 `EINVAL`。
pub fn sys_sigaction(
    signum: usize, action: *const SignalAction, old_action: *mut SignalAction,
) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sigaction",
        current_task().unwrap().pid.0,
        current_task().unwrap().tid
    );
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access(file!(), line!());
    if signum > MAX_SIG {
        error!("[sys_sigaction] error signum");
        return EPERM;
    }
    if old_action as usize != 0 {
        unsafe { sstatus::set_sum() };
        unsafe { *old_action = inner.signal_actions.table[signum].clone() };
        unsafe { sstatus::clear_sum() };
    }
    if let Some(flag) = SignalFlags::from_bits(1 << (signum - 1)) {
        if check_sigaction_error(flag) {
            error!("[sys_sigaction] check_sigaction_error");
            return EPERM;
        }
        let old_kernel_action = inner.signal_actions.table[signum];
        if old_action as usize != 0 {
            if old_kernel_action.mask != SignalFlags::from_bits(40).unwrap() {
                unsafe { sstatus::set_sum() };
                unsafe { *old_action = old_kernel_action };
                unsafe { sstatus::clear_sum() };
            } else {
                unsafe { sstatus::set_sum() };
                let mut ref_old_action = unsafe { *old_action };
                unsafe { sstatus::clear_sum() };
                ref_old_action.sa_handler = old_kernel_action.sa_handler;
            }
        }
        if action as usize != 0 {
            unsafe { sstatus::set_sum() };
            let ref_action = unsafe { &*action };
            inner.signal_actions.table[signum as usize] = *ref_action;
            unsafe { sstatus::clear_sum() };
        }
        return SUCCESS;
    } else {
        println!("Undefined SignalFlags");
        return EPERM;
    }
}

fn check_sigaction_error(signal: SignalFlags) -> bool {
    if signal == SignalFlags::SIGKILL || signal == SignalFlags::SIGSTOP {
        true
    } else {
        false
    }
}

// The timedwait used in the libtest is different from the linux manual page
pub fn sys_sigtimedwait(
    uthese: *mut usize,
    info: *mut SigInfo,
    uts: *const TimeSpec,
    // I find sigsetsize in Linux 5.2 source code, but I dont know how to use it.
    sigsetsize: usize,
) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sigtimedwait",
        current_task().unwrap().pid.0,
        current_task().unwrap().tid
    );

    // if uthese as usize == 0 || uts as usize == 0 {
    //     error!("[sys_sigtimedwait] Null pointer.");
    //     return EPERM;
    // }
    // let mut timeout: TimeSpec = TimeSpec::now();
    // unsafe {
    //     sstatus::set_sum();
    //     timeout = *uts;
    //     sstatus::clear_sum();
    // }

    // let limit_time = TimeSpec::now() + timeout;

    // let mut set = 0;
    // unsafe {
    //     sstatus::set_sum();
    //     set = *uthese;
    //     sstatus::clear_sum();
    // }

    // let set_flags = SignalFlags::from_bits(set).unwrap();

    // loop {
    //     let task = current_task().unwrap();
    //     let signals_pending = task
    //         .inner_exclusive_access(file!(), line!())
    //         .signals_pending;
    //     // Every matched signals will return. This method is wrong.
    //     let match_signals = set_flags & signals_pending;
    //     if !match_signals.is_empty() {
    //         let first_signals = match_signals.bits().trailing_zeros();
    //         if info as usize != 0 {
    //             let siginfo = SigInfo::new(first_signals as usize, 0, 0);
    //             unsafe {
    //                 sstatus::set_sum();
    //                 *info = siginfo;
    //                 sstatus::clear_sum();
    //             }
    //         }
    //         return SUCCESS;
    //     }
    //     if limit_time < TimeSpec::now() {
    //         println!("[sys_sigtimedwait] Timeout.");
    //         return EAGAIN;
    //     }
    //     drop(task);
    //     drop(signals_pending);
    //     debug!("sigtimedwait: suspend_current_and_run_next");
    //     suspend_current_and_run_next();
    // }
    SUCCESS
}
