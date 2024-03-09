use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use prost::Message;
use proto::backend::{Commit, File};
use tracing::info;

use crate::content_hash::{blake3, ContentHash};

pub type Id = [u8; 32];

/// Index Node Number
pub type Inode = u64;

pub type DirectoryDescriptor = BTreeMap<Vec<u8>, (Inode, FileKind)>;

pub struct MountStore {}

impl MountStore {
    pub fn new() -> Self {
        MountStore {}
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InodeAttributes {
    inode: Inode,
    open_file_handles: u64, // Ref count of open file handles to this inode
    size: u64,
    last_accessed: (i64, u32),
    last_modified: (i64, u32),
    last_metadata_changed: (i64, u32),
    kind: FileKind,
    // Permissions and special mode bits
    mode: u16,
    hardlinks: u32,
    uid: u32,
    gid: u32,
    xattrs: BTreeMap<Vec<u8>, Vec<u8>>,
}

fn time_now() -> (i64, u32) {
    time_from_system_time(&SystemTime::now())
}

fn time_from_system_time(system_time: &SystemTime) -> (i64, u32) {
    // Convert to signed 64-bit time with epoch at 0
    match system_time.duration_since(UNIX_EPOCH) {
        Ok(duration) => (duration.as_secs() as i64, duration.subsec_nanos()),
        Err(before_epoch_error) => (
            -(before_epoch_error.duration().as_secs() as i64),
            before_epoch_error.duration().subsec_nanos(),
        ),
    }
}

impl InodeAttributes {
    pub fn get_inode(&self) -> Inode {
        self.inode
    }

    pub fn get_mode(&self) -> u16 {
        self.mode
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn get_last_metadata_changed(&self) -> (i64, u32) {
        self.last_metadata_changed
    }

    pub fn get_last_modified(&self) -> (i64, u32) {
        self.last_modified
    }

    pub fn get_last_accessed(&self) -> (i64, u32) {
        self.last_accessed
    }

    pub fn get_hardlinks(&self) -> u32 {
        self.hardlinks
    }

    pub fn get_uid(&self) -> u32 {
        self.uid
    }

    pub fn get_gid(&self) -> u32 {
        self.gid
    }

    pub fn get_kind(&self) -> FileKind {
        self.kind
    }

    pub fn from_tree_id(inode: Inode, id: Id) -> InodeAttributes {
        InodeAttributes {
            inode,
            open_file_handles: 0,
            size: 0,
            last_accessed: time_now(),
            last_modified: time_now(),
            last_metadata_changed: time_now(),
            kind: FileKind::Directory,
            mode: 0o777,
            hardlinks: 2,
            uid: 0,
            gid: 0,
            xattrs: Default::default(),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum FileKind {
    File,
    Directory,
    Symlink,
}
