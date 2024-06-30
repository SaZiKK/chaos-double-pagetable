//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`task`]: Task management
//! - [`syscall`]: System call handling and implementation
//! - [`mm`]: Address map using SV39
//! - [`sync`]: Wrap a static data structure inside it so that we are able to access it without any `unsafe`.
//! - [`fs`]: Separate user from file system with some structures
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`task::run_tasks()`] and for the first time go to
//! userspace.

#![no_std]
#![no_main]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(ascii_char)]
#![feature(negative_impls)]

use core::arch::{asm, global_asm};

use board::QEMUExit;

#[macro_use]
extern crate log;

extern crate alloc;

#[macro_use]
extern crate bitflags;

#[path = "boards/qemu.rs"]
mod board;

#[macro_use]
mod console;
pub mod block;
pub mod config;
pub mod drivers;
pub mod fs;
pub mod lang_items;
pub mod logging;
pub mod mm;
pub mod sbi;
pub mod sync;
pub mod syscall;
pub mod task;
pub mod timer;
pub mod trap;
pub mod utils;

use config::KERNEL_SPACE_OFFSET;

global_asm!(include_str!("entry.S"));

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

#[no_mangle]
fn show_logo() {
    println!(
        r#"
 .d88888b.                     .d88888b.   .d8888b.
d88P" "Y88b 888               d88P" "Y88b d88P  Y88b
888     888 888               888     888 Y88b.
888         888d88b.  .d88b.8 888     888  "Y888b.
888         888PY888 d8P""Y88 888     888     "Y88b.
888     888 888  888 888  888 888     888       "888
Y88b. .d88P 888  888 Y8b..d88 Y88b. .d88P Y88b  d88P
 "Y88888P"  888  888  "Y88P`8b "Y88888P"   "Y8888P" 
"#
    );
}

const ALL_TASKS: [&str; 32] = [
    "read",
    "clone",
    "write",
    "dup2",
    "times",
    "uname",
    "wait",
    "gettimeofday",
    "waitpid",
    "brk",
    "getpid",
    "fork",
    "close",
    "dup",
    "exit",
    "sleep",
    "yield",
    "getppid",
    "open",
    "openat",
    "getcwd",
    "execve",
    "mkdir_",
    "chdir",
    "fstat",
    "mmap",
    "munmap",
    "pipe",
    "mount",
    "umount",
    "getdents",
    "unlink",
];

#[no_mangle]
pub fn fake_main() {
    unsafe {
        asm!("add sp, sp, {}", in(reg) KERNEL_SPACE_OFFSET << 12);
        asm!("la t0, rust_main");
        asm!("add t0, t0, {}", in(reg) KERNEL_SPACE_OFFSET << 12);
        asm!("jalr zero, 0(t0)");
    }
}

#[no_mangle]
/// the rust entry-point of os
pub fn rust_main() -> ! {
    show_logo();
    clear_bss();
    debug!("clear bss section done");
    println!("[kernel] Hello, world!");
    logging::init();
    debug!("logging init done");
    mm::init();
    debug!("mm init done");
    mm::remap_test();
    debug!("mm remap test done");
    trap::init();
    debug!("trap init done");
    trap::enable_timer_interrupt();
    debug!("timer interrupt enabled");
    timer::set_next_trigger();
    debug!("timer set next trigger done");
    // for file in ROOT_INODE.ls() {
    //     println!("{}", file);
    // }
    for file in ALL_TASKS.iter() {
        task::add_file(file);
        task::run_tasks();
    }
    println!("All tasks finished successfully!");
    println!("ChaOS is shutting down...");
    crate::board::QEMU_EXIT_HANDLE.exit_success();
}
