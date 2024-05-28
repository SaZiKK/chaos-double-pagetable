use core::cmp::min;

use alloc::{string::String, sync::Arc, vec::Vec};
use spin::Mutex;

use crate::block::{block_cache::{block_cache_sync_all, get_block_cache}, block_dev::BlockDevice, BLOCK_SZ};

use super::{dentry::{self, Fat32Dentry, Fat32DentryLayout, Fat32LDentryLayout, FileAttributes}, fat::FAT, inode::{Fat32Inode, Fat32InodeType}, super_block::{Fat32SB, Fat32SBLayout}};

pub struct Fat32FS {
    pub sb: Fat32SB,
    pub fat: Arc<FAT>,
    pub bdev: Arc<dyn BlockDevice>,
}

impl Fat32FS {
    /// load a exist fat32 file system from block device
    pub fn load(bdev: Arc<dyn BlockDevice>) -> Arc<Mutex<Self>> {
        get_block_cache(0, Arc::clone(&bdev))
            .lock()
            .read(0, |sb_layout: &Fat32SBLayout| {
                assert!(sb_layout.is_valid(), "Error loading FAT32!");
                let fat32fs = Self {
                    sb: Fat32SB::from_layout(sb_layout),
                    fat: Arc::new(FAT::from_sb(Arc::new(Fat32SB::from_layout(sb_layout)), &bdev)),
                    bdev,
                };
                Arc::new(Mutex::new(fat32fs))
            })
    }

    /// get root inode
    pub fn root_inode(fs: &Arc<Mutex<Fat32FS>>) -> Fat32Inode {
        let fs_ = fs.lock();
        let start_cluster = fs_.sb.root_cluster as usize;
        let bdev = Arc::clone(&fs_.bdev);
        Fat32Inode {
            type_: Fat32InodeType::Dir,
            start_cluster,
            fs: Arc::clone(fs),
            bdev: Arc::clone(&bdev),
            dentry: Arc::new(Mutex::new(Fat32Dentry::new(0, 0, &bdev, &fs_.fat))),
        }
    }

    /// get cluster chain
    pub fn cluster_chain(&self, start_cluster: usize) -> Vec<usize> {
        let mut cluster_chain = Vec::new();
        let mut cluster = start_cluster;
        loop {
            cluster_chain.push(cluster);
            if let Some(next_cluster) = self.fat.next_cluster_id(cluster) {
                cluster = next_cluster;
            } else {
                break;
            }
        }
        cluster_chain
    }

    /// read a cluster
    pub fn read_cluster(&self, cluster: usize, buf: &mut [u8; 4096]) {
        let cluster_offset = self.sb.root_sector() + (cluster - 2) * self.sb.sectors_per_cluster as usize;
        let cluster_size = self.sb.bytes_per_sector as usize * self.sb.sectors_per_cluster as usize;
        let mut read_size = 0;
        for i in 0..self.sb.sectors_per_cluster {
            get_block_cache(cluster_offset as usize + i as usize, Arc::clone(&self.bdev))
                .lock()
                .read(0, |data: &[u8; BLOCK_SZ as usize]| {
                    let copy_size = core::cmp::min(cluster_size - read_size, data.len());
                    buf[read_size..read_size + copy_size].copy_from_slice(&data[..copy_size]);
                    read_size += copy_size;
                });
        }
    }

    /// write a cluster
    pub fn write_cluster(&self, cluster: usize, buf: &[u8; 4096]) {
        let cluster_offset = self.sb.root_sector() + (cluster - 2) * self.sb.sectors_per_cluster as usize;
        let cluster_size = self.sb.bytes_per_sector as usize * self.sb.sectors_per_cluster as usize;
        let mut write_size = 0;
        for i in 0..self.sb.sectors_per_cluster {
            get_block_cache(cluster_offset as usize + i as usize, Arc::clone(&self.bdev))
                .lock()
                .modify(0, |data: &mut [u8; BLOCK_SZ as usize]| {
                    let copy_size = core::cmp::min(cluster_size - write_size, data.len());
                    data[..copy_size].copy_from_slice(&buf[write_size..write_size + copy_size]);
                    write_size += copy_size;
                });
        }
    }

    /// get next dentry sector id and offset
    pub fn next_dentry_id(&self, sector_id: usize, offset: usize) -> Option<(usize, usize)> {
        if offset >= 512 || offset % 32 != 0 {
            return None;
        }
        let next_offset = offset + 32;
        if next_offset >= 512 {
            let next_sector_id = sector_id + 1;
            if next_sector_id % self.sb.sectors_per_cluster as usize == 0 {
                if let Some(next_sector_id) = self.fat.next_cluster_id(sector_id) {
                    Some((next_sector_id, 0))
                } else {
                    None
                }
            } else {
                Some((next_sector_id, 0))
            }
        } else {
            Some((sector_id, next_offset))
        }
    }

    /// get a dentry with sector id and offset
    pub fn get_dentry(&self, sector_id: &mut usize, offset: &mut usize) -> Option<Fat32Dentry> {
        if *offset >= 512 || *offset % 32 != 0 {
            return None;
        }
        let mut is_long_entry = false;
        let dentry = get_block_cache(*sector_id as usize, Arc::clone(&self.bdev))
            .lock()
            .read(*offset, |layout: &Fat32DentryLayout| {
                if layout.is_deleted() {
                    return Fat32Dentry::new_deleted(&self.bdev, &self.fat);
                }
                if layout.is_long() {
                    is_long_entry = true;
                }
                Fat32Dentry::new(*sector_id, *offset, &self.bdev, &self.fat)
            });
        if is_long_entry {
            let mut is_end = false;
            loop {
                get_block_cache(*sector_id as usize, Arc::clone(&self.bdev))
                    .lock()
                    .read(*offset, |layout: &Fat32LDentryLayout| {
                        if layout.is_end() {
                            is_end = true;
                        }
                    });
                (*sector_id, *offset) = self.next_dentry_id(*sector_id, *offset).unwrap();
                if is_end {
                    break;
                }
            }
        }
        (*sector_id, *offset) = self.next_dentry_id(*sector_id, *offset).unwrap();
        Some(dentry)
    }
    
    pub fn insert_dentry(&self, cluster_id: usize, name: String, attr: FileAttributes, file_size: u32, start_cluster: usize) -> Option<Fat32Dentry> {
        let mut sector_id = self.fat.cluster_id_to_sector_id(cluster_id).unwrap();
        let mut offset = 0;
        let mut found = false;
        while !found {
            get_block_cache(sector_id, self.bdev.clone())
                .lock()
                .read(offset, |layout: &Fat32DentryLayout| {
                    if layout.is_empty() {
                        found = true;
                    }
                })
        }
        let mut order = 1;
        let mut pos = 0;
        while pos < name.len() {
            let copy_len = min(13, name.len() - pos);
            get_block_cache(sector_id as usize, Arc::clone(&self.bdev))
                .lock()
                .modify(offset, |layout: &mut Fat32LDentryLayout| {
                    *layout = Fat32LDentryLayout::new(
                        order,
                        &name[pos..pos + copy_len],
                        pos + copy_len == name.len(),
                    );
                });
            order += 1;
            pos += copy_len;
            (sector_id, offset) = self.next_dentry_id(sector_id, offset).unwrap();
        }
        get_block_cache(sector_id as usize, self.bdev.clone())
            .lock()
            .modify(offset, | layout: &mut Fat32DentryLayout | {
                *layout = Fat32DentryLayout::new(
                    name.as_str(),
                    attr,
                    start_cluster,
                    file_size,
                );
            });
        Some(Fat32Dentry::new(sector_id, offset, &self.bdev, &self.fat))
    }

    pub fn remove_dentry(&self, dentry: &Fat32Dentry) {
        let mut sector_id = dentry.sector_id;
        let mut offset = dentry.sector_offset;
        if dentry.is_long()   {
            let mut is_end = false;
            loop {
                get_block_cache(sector_id, Arc::clone(&self.bdev))
                    .lock()
                    .modify(offset, |layout: &mut Fat32LDentryLayout| {
                        layout.order = 0xE5;
                        if layout.is_end() {
                            is_end = true;
                        }
                    });
                (sector_id, offset) = self.next_dentry_id(sector_id, offset).unwrap();
                if is_end {
                    break;
                }
            }
        }
        get_block_cache(sector_id, self.bdev.clone())
            .lock()
            .modify(offset, |layout: &mut Fat32DentryLayout| {
                layout.set_deleted();
            });
    }
}