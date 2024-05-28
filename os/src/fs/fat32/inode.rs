use core::cmp::min;

use alloc::{string::{String, ToString}, sync::Arc, vec::Vec};
use spin::Mutex;

use crate::{block::block_dev::BlockDevice, fs::{fat32::CLUSTER_SIZE, inode::{Inode, OSInode, Stat, StatMode, ROOT_INODE}}};

use super::{dentry::{self, Fat32Dentry, FileAttributes}, file_system::Fat32FS};

#[derive(Clone)]
pub struct Fat32Inode {
    pub type_: Fat32InodeType,
    pub start_cluster: u32,
    pub fize_size: u32,
    pub fs: Arc<Mutex<Fat32FS>>,
    pub bdev: Arc<dyn BlockDevice>,
}

impl Inode for Fat32Inode {
    fn fstat(self: Arc<Self>) -> Stat {
        let mode = match self.type_ {
            Fat32InodeType::File => StatMode::FILE,
            Fat32InodeType::Dir => StatMode::DIR,
            _ => StatMode::NULL,
            
        };
        Stat::new(
            mode,
            0,
        )
    }
    
    fn find(self: Arc<Self>, path: &str) -> Option<Arc<dyn Inode>> {
        let mut split = path.splitn(2, '/');
        let name = split.next().unwrap();
        let next = split.next();
        if name == "." {
            if next.is_some() {
                return self.find(next.unwrap());
            } else {
                return Some(self);
            }
        }
        let fs = self.fs.lock();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            let type_ = if dentry.is_file() {
                Fat32InodeType::File
            } else if dentry.is_dir() {
                Fat32InodeType::Dir
            } else {
                Fat32InodeType::VolumeId
            };
            if dentry.name() == name {
                let inode = Arc::new(Fat32Inode {
                    type_,
                    start_cluster: dentry.start_cluster(),
                    fize_size: dentry.file_size(),
                    fs: Arc::clone(&self.fs),
                    bdev: Arc::clone(&self.bdev),
                });
                if next.is_some() {
                    return Arc::clone(&inode).find(split.next().unwrap());
                } else {
                    return Some(inode);
                }
            }
        }
        None
    }
    
    fn create(self: Arc<Self>, name: &str, stat: StatMode) -> Option<Arc<dyn Inode>> {
        if self.clone().find(name).is_some() {
            return None;
        }
        let fs = self.fs.lock();
        let mut next_sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut next_offset = 0;
        let mut sector_id = next_sector_id;
        let mut offset = next_offset;
        while let Some(_) = fs.get_dentry(&mut next_sector_id, &mut next_offset) {
            sector_id = next_sector_id;
            offset = next_offset;
        }
        let attr = match stat {
            StatMode::FILE => FileAttributes::ARCHIVE,
            StatMode::DIR => FileAttributes::DIRECTORY,
            _ => FileAttributes::ARCHIVE,
        };
        let start_cluster = fs.fat.alloc_new_cluster(&self.bdev).unwrap();
        let dentry = Fat32Dentry::new(
            name.to_string(),
            attr,
            0,
            start_cluster,
        );
        fs.insert_dentry(sector_id, offset, dentry);
        let type_ = if stat == StatMode::FILE {
            Fat32InodeType::File
        } else {
            Fat32InodeType::Dir
        };
        Some(Arc::new(Fat32Inode {
                type_,
                start_cluster,
                fize_size: 0,
                fs: Arc::clone(&self.fs),
                bdev: Arc::clone(&self.bdev),
            })
        )
    }
    
    fn link(self: Arc<Self>, old_name: &str, new_name: &str) -> Option<Arc<dyn Inode>> {
        todo!()
    }
    
    fn unlink(self: Arc<Self>, path: &str) -> bool {
        let mut split = path.splitn(2, '/');
        let name = split.next().unwrap();
        let next = split.next();
        if name == "." {
            if next.is_some() {
                return self.unlink(next.unwrap());
            } else {
                return false;
            }
        }
        let fs = self.fs.lock();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            if dentry.name() == name {
                if next.is_some() {
                    let inode = Arc::new(Fat32Inode {
                        type_: if dentry.is_file() {
                            Fat32InodeType::File
                        } else if dentry.is_dir() {
                            Fat32InodeType::Dir
                        } else {
                            Fat32InodeType::VolumeId
                        },
                        start_cluster: dentry.start_cluster(),
                        fize_size: dentry.file_size(),
                        fs: Arc::clone(&self.fs),
                        bdev: Arc::clone(&self.bdev),
                    });
                    return inode.unlink(next.unwrap());
                } else {
                    fs.remove_dentry(sector_id, offset);
                    return true;
                }
            }
        }
        true
    }
    
    fn ls(self: Arc<Self>) -> Vec<String> {
        let fs = self.fs.lock();
        let mut v = Vec::new();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            v.push(dentry.name());
        }
        v
    }
    
    fn read_at(self: Arc<Self>, offset: usize, buf: &mut [u8]) -> usize {
        let fs = self.fs.lock();
        let cluster_id = self.start_cluster;
        let cluster_chain = fs.cluster_chain(cluster_id);
        let mut read_size = 0;
        let mut pos = 0;
        let mut cluster_buf = [0u8; CLUSTER_SIZE];
        for cluster_id in cluster_chain {
            if pos < offset {
                let pass_size = min(CLUSTER_SIZE, offset - pos);
                pos += pass_size;
                if pass_size == CLUSTER_SIZE {
                    continue;
                }
            }
            fs.read_cluster(cluster_id, &mut cluster_buf);
            let copy_size = min(self.fize_size as usize - pos, buf.len() - read_size);
            buf[read_size..read_size + copy_size].copy_from_slice(&cluster_buf[pos % CLUSTER_SIZE..pos % CLUSTER_SIZE + copy_size]);
            read_size += copy_size;
            pos += copy_size;
            if read_size >= buf.len() || pos >= self.fize_size as usize {
                break;
            }
        }
        read_size
    }
    
    fn write_at(self: Arc<Self>, offset: usize, buf: &[u8]) -> usize {
        let fs = self.fs.lock();
        let cluster_id = self.start_cluster;
        let cluster_chain = fs.cluster_chain(cluster_id);
        let mut write_size = 0;
        let mut pos = 0;
        let mut cluster_buf = [0u8; CLUSTER_SIZE];
        for cluster_id in cluster_chain {
            if pos < offset {
                let pass_size = min(CLUSTER_SIZE, offset - pos);
                pos += pass_size;
                if pass_size == CLUSTER_SIZE {
                    continue;
                }
            }
            fs.read_cluster(cluster_id, &mut cluster_buf);
            let copy_size = min(buf.len() - write_size, CLUSTER_SIZE - pos % CLUSTER_SIZE);
            cluster_buf[pos % CLUSTER_SIZE..pos % CLUSTER_SIZE + copy_size].copy_from_slice(&buf[write_size..write_size + copy_size]);
            fs.write_cluster(cluster_id, &cluster_buf);
            write_size += copy_size;
            pos += copy_size;
            if write_size >= buf.len() {
                break;
            }
        }
        write_size
    }
    
    fn clear(self: Arc<Self>) {
        todo!()
    } 

    /// Get the current directory name
    fn current_dirname(self: Arc<Self>) -> Option<String> {
        if self.type_ != Fat32InodeType::Dir {
            return None;
        }
        let fs = self.fs.lock();
        let mut sector_id = fs.fat.cluster_id_to_sector_id(self.start_cluster).unwrap();
        let mut offset = 0;
        while let Some(dentry) = fs.get_dentry(&mut sector_id, &mut offset) {
            if dentry.name() == "." {
                return Some(dentry.name());
            }
        }
        None
    }

}

impl Fat32Inode {
    
    pub fn is_dir(&self) -> bool {
        self.type_ == Fat32InodeType::Dir
    }

    pub fn is_file(&self) -> bool {
        self.type_ == Fat32InodeType::File
    }

}

#[derive(PartialEq, Clone, Copy)]
pub enum Fat32InodeType {
    File,
    Dir,
    VolumeId,
}