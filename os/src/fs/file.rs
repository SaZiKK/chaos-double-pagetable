use alloc::vec::Vec;
use core::any::Any;

use super::inode::Stat;
use crate::mm::UserBuffer;

/// trait File for all file types
pub trait File: Any + Send + Sync {
    /// the file readable?
    fn readable(&self) -> bool;
    /// the file writable?
    fn writable(&self) -> bool;
    /// read from the file to buf, return the number of bytes read
    fn read(&self, buf: UserBuffer) -> usize;
    /// read all data from the file
    fn read_all(&self) -> Vec<u8>;
    /// write to the file from buf, return the number of bytes writte
    fn write(&self, buf: UserBuffer) -> usize;
    /// get file status
    fn fstat(&self) -> Option<Stat>;
    /// is directory
    fn is_dir(&self) -> bool {
        if let Some(stat) = self.fstat() {
            stat.is_dir()
        } else {
            true
        }
    }
}
