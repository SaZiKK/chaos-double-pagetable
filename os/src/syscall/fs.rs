use core::borrow::Borrow;
use core::mem::size_of;
use core::ptr;

use crate::fs::file::File;
use crate::fs::inode::{Inode, OSInode, Stat, ROOT_INODE};
use crate::fs::{link, make_pipe, open_file, unlink, OpenFlags};
use crate::mm::{translated_byte_buffer, translated_refmut, translated_str, UserBuffer};
use crate::syscall::process;
use crate::task::{current_process, current_task, current_user_token};
use alloc::sync::Arc;

pub const AT_FDCWD: i32 = -100;

/// write syscall
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_write",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}
/// read syscall
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_read",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let token = current_user_token();
    let process = current_process();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}
/// openat sys
pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!(
        "kernel:pid[{}] sys_open",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let token = current_user_token();
    let path = translated_str(token, path);
    debug!("open file: {}, flags: {:#x}", path.as_str(), flags);
    if let Some(inode) = open_file(ROOT_INODE.as_ref(), path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = process.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}
pub fn sys_openat(dirfd: i32, path: *const u8, flags: u32) -> isize {
    trace!(
        "kernel:pid[{}] sys_open",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    if dirfd == AT_FDCWD {
        return sys_open(path, flags);
    }
    debug!("openat: dirfd: {}, path: {:?}, flags: {:#x}", dirfd, path, flags);
    let dirfd = dirfd as usize;
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if dirfd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[dirfd].is_none() {
        return -1;
    }
    let dir = inner.fd_table[dirfd].as_ref().unwrap().clone();
    if !dir.is_dir() {
        return -1;
    }
    let inode = unsafe { &*(dir.as_ref() as *const dyn File as *const OSInode) };
    let token = inner.memory_set.token();
    let path = translated_str(token, path);
    debug!("open file: {}", path.as_str());
    if let Some(inode) = open_file(inode, path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}
/// close syscall
pub fn sys_close(fd: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_close",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}
/// pipe syscall
pub fn sys_pipe(pipe: *mut usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_pipe",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let token = current_user_token();
    let mut inner = process.inner_exclusive_access();
    let (pipe_read, pipe_write) = make_pipe();
    let read_fd = inner.alloc_fd();
    inner.fd_table[read_fd] = Some(pipe_read);
    let write_fd = inner.alloc_fd();
    inner.fd_table[write_fd] = Some(pipe_write);
    *translated_refmut(token, pipe) = read_fd;
    *translated_refmut(token, unsafe { pipe.add(1) }) = write_fd;
    0
}
/// dup syscall
pub fn sys_dup(fd: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_dup",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    let new_fd = inner.alloc_fd();
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}

/// dup3 syscall
pub fn sys_dup3(fd: usize, new_fd: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_dup3",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let process = current_process();
    let mut inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    while inner.fd_table.len() <= new_fd {
        inner.fd_table.push(None);
    }
    if inner.fd_table[new_fd].is_some() {
        return -1;
    }
    inner.fd_table[new_fd] = Some(Arc::clone(inner.fd_table[fd].as_ref().unwrap()));
    new_fd as isize
}

/// YOUR JOB: Implement fstat.
pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let task = current_task().unwrap();
    let process = task.process.upgrade().unwrap();
    let inner = process.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        let stat = file.fstat();
        if stat.is_none() {
            return -1;
        }
        let stat = stat.unwrap();
        let mut v = translated_byte_buffer(inner.get_user_token(), st as *const u8, size_of::<Stat>());
        unsafe {
            let mut p = stat.borrow() as *const Stat as *const u8;
            for slice in v.iter_mut() {
                let len = slice.len();
                ptr::copy_nonoverlapping(p, slice.as_mut_ptr(), len);
                p = p.add(len);
            }
        }
    }
    0
}

/// YOUR JOB: Implement linkat.
pub fn sys_linkat(old_name: *const u8, new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let token = current_user_token();
    let old_name = translated_str(token, old_name);
    let new_name = translated_str(token, new_name);
    if link(old_name.as_str(), new_name.as_str()).is_some() {
        0
    } else {
        -1
    }
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let token = current_user_token();
    let name = translated_str(token, name);
    if unlink(name.as_str()) {
        0
    } else {
        -1
    }
}

pub fn sys_getcwd(buf: *mut u8, len: usize) -> isize {
    trace!(
        "kernel:pid[{}] sys_getcwd",
        current_task().unwrap().process.upgrade().unwrap().getpid()
    );
    let token = current_user_token();
    if let Some(path) = current_task().unwrap().inner_exclusive_access().work_dir.clone().current_dirname() {
        let len = core::cmp::min(len, path.len());
        let mut v = translated_byte_buffer(token, buf, len);
        unsafe {
            let mut p = path.as_bytes().as_ptr();
            for slice in v.iter_mut() {
                let len = slice.len();
                ptr::copy_nonoverlapping(p, slice.as_mut_ptr(), len);
                p = p.add(len);
            }
        }
        buf as isize
    } else {
        -1
    }
}