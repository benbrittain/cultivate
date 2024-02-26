use std::{
    collections::{BTreeMap, HashMap},
    ffi::{c_int, OsStr},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    sync::{atomic::AtomicU64, Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Error};
use fuser::{
    Filesystem, KernelConfig, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request,
};

use crate::store::{DirectoryDescriptor, FileKind, Inode, InodeAttributes, Store};

const BLOCK_SIZE: u64 = 512;

use fuser::FUSE_ROOT_ID;

fn system_time_from_time(secs: i64, nsecs: u32) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::new(secs as u64, nsecs)
    } else {
        UNIX_EPOCH - Duration::new((-secs) as u64, nsecs)
    }
}

impl From<InodeAttributes> for fuser::FileAttr {
    fn from(attrs: InodeAttributes) -> Self {
        fuser::FileAttr {
            ino: attrs.get_inode(),
            size: attrs.get_size(),
            blocks: (attrs.get_size() + BLOCK_SIZE - 1) / BLOCK_SIZE,
            atime: system_time_from_time(attrs.get_last_accessed().0, attrs.get_last_accessed().1),
            mtime: system_time_from_time(attrs.get_last_modified().0, attrs.get_last_modified().1),
            ctime: system_time_from_time(
                attrs.get_last_metadata_changed().0,
                attrs.get_last_metadata_changed().1,
            ),
            crtime: SystemTime::UNIX_EPOCH,
            kind: attrs.get_kind().into(),
            perm: attrs.get_mode(),
            nlink: attrs.get_hardlinks(),
            uid: attrs.get_uid(),
            gid: attrs.get_gid(),
            rdev: 0,
            blksize: BLOCK_SIZE as u32,
            flags: 0,
        }
    }
}

impl From<FileKind> for fuser::FileType {
    fn from(kind: FileKind) -> Self {
        match kind {
            FileKind::File => fuser::FileType::RegularFile,
            FileKind::Directory => fuser::FileType::Directory,
            FileKind::Symlink => fuser::FileType::Symlink,
        }
    }
}

struct CultivateFS {
    store: Store,
}

impl CultivateFS {
    pub fn new(store: Store) -> Self {
        CultivateFS { store }
    }

    fn get_inode(&self, inode: Inode) -> Result<InodeAttributes, libc::c_int> {
        if let Some(attr) = self.store.get_inode(inode) {
            return Ok(attr.clone());
        }
        Err(libc::ENOENT)
    }

    fn write_inode(&self, attrs: &InodeAttributes) {
        self.store.write_inode(attrs.clone())
    }

    fn write_directory_content(&self, inode: Inode, entries: DirectoryDescriptor) {
        self.store.write_directory_content(inode.clone(), entries)
    }

    fn get_directory_content(&self, inode: Inode) -> Result<DirectoryDescriptor, libc::c_int> {
        if let Some(attr) = self.store.get_directory_content(inode) {
            return Ok(attr.clone());
        }
        Err(libc::ENOENT)
    }

    fn lookup_name(&self, parent: u64, name: &OsStr) -> Result<InodeAttributes, c_int> {
        let entries = self.get_directory_content(parent)?;
        if let Some((inode, _)) = entries.get(name.as_bytes()) {
            self.get_inode(*inode)
        } else {
            Err(libc::ENOENT)
        }
    }
}

pub struct MountManager {
    store: Store,
}

impl MountManager {
    pub fn new(store: Store) -> Self {
        MountManager { store }
    }

    pub fn mount<P: Into<PathBuf> + std::fmt::Debug>(
        &self,
        mountpoint: P,
    ) -> Result<fuser::BackgroundSession, Error> {
        let mountpoint = mountpoint.into();

        let mut options = vec![
            MountOption::FSName("cultivate".to_string()),
            MountOption::AutoUnmount,
        ];
        if mountpoint.is_dir() {
            fuser::spawn_mount2(CultivateFS::new(self.store.clone()), mountpoint, &options)
                .map_err(Into::into)
        } else {
            Err(anyhow!("No directory to mount filesystem at exists"))
        }
    }
}

impl Filesystem for CultivateFS {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        //if name.len() > MAX_NAME_LENGTH as usize {
        //    reply.error(libc::ENAMETOOLONG);
        //    return;
        //}
        //let parent_attrs = self.get_inode(parent).unwrap();
        //if !check_access(
        //    parent_attrs.uid,
        //    parent_attrs.gid,
        //    parent_attrs.mode,
        //    req.uid(),
        //    req.gid(),
        //    libc::X_OK,
        //) {
        //    reply.error(libc::EACCES);
        //    return;
        //}

        match self.lookup_name(parent, name) {
            Ok(attrs) => reply.entry(&Duration::new(0, 0), &attrs.into(), 0),
            Err(error_code) => reply.error(error_code),
        }
    }

    fn init(
        &mut self,
        req: &Request,
        #[allow(unused_variables)] config: &mut KernelConfig,
    ) -> Result<(), libc::c_int> {
        if self.get_inode(FUSE_ROOT_ID).is_err() {
            let root = InodeAttributes::from_tree_id(FUSE_ROOT_ID, self.store.get_root_tree_id());
            self.write_inode(&root);
            let mut entries = BTreeMap::new();
            entries.insert(b".".to_vec(), (FUSE_ROOT_ID, FileKind::Directory));
            self.store.write_directory_content(FUSE_ROOT_ID, entries);
        }
        Ok(())
    }

    //fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
    //    dbg!("statfs() implementation is a stub");
    //}

    //fn access(&mut self, req: &Request, inode: u64, mask: i32, reply: ReplyEmpty) {
    //    dbg!("access() called with {:?} {:?}", inode, mask);
    //    // TODO access control
    //    reply.ok();
    //}

    fn readdir(
        &mut self,
        _req: &Request,
        inode: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        dbg!("readdir() called with {:?}", inode);
        assert!(offset >= 0);
        let entries = match self.get_directory_content(inode) {
            Ok(entries) => entries,
            Err(error_code) => {
                reply.error(error_code);
                return;
            }
        };

        for (index, entry) in entries.iter().skip(offset as usize).enumerate() {
            let (name, (inode, file_type)) = entry;

            let buffer_full: bool = reply.add(
                *inode,
                offset + index as i64 + 1,
                (*file_type).into(),
                OsStr::from_bytes(name),
            );

            if buffer_full {
                break;
            }
        }

        reply.ok();
    }

    //fn opendir(&mut self, req: &Request, inode: u64, flags: i32, reply: ReplyOpen) {
    //    dbg!("opendir() called on {:?}", inode);
    //}

    //fn open(&mut self, req: &Request, inode: u64, flags: i32, reply: ReplyOpen) {
    //    dbg!("open() called for {:?}", inode);
    //}

    //fn read(
    //    &mut self,
    //    _req: &Request,
    //    inode: u64,
    //    fh: u64,
    //    offset: i64,
    //    size: u32,
    //    _flags: i32,
    //    _lock_owner: Option<u64>,
    //    reply: ReplyData,
    //) {
    //    dbg!(
    //        "read() called on {:?} offset={:?} size={:?}",
    //        inode,
    //        offset,
    //        size
    //    );
    //    assert!(offset >= 0);
    //}

    //fn write(
    //    &mut self,
    //    _req: &Request,
    //    inode: u64,
    //    fh: u64,
    //    offset: i64,
    //    data: &[u8],
    //    _write_flags: u32,
    //    #[allow(unused_variables)] flags: i32,
    //    _lock_owner: Option<u64>,
    //    reply: ReplyWrite,
    //) {
    //    dbg!("write() called with {:?} size={:?}", inode, data.len());
    //    assert!(offset >= 0);
    //}

    //fn forget(&mut self, _req: &Request, _ino: u64, _nlookup: u64) {}

    fn getattr(&mut self, _req: &Request, inode: u64, reply: ReplyAttr) {
        match self.get_inode(inode) {
            Ok(attrs) => reply.attr(&Duration::new(0, 0), &attrs.into()),
            Err(error_code) => reply.error(error_code),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::mpsc::channel};

    use super::*;

    fn setup_mount(func: fn(PathBuf, Store)) {
        let (start_tx, start_rx) = channel();
        let (end_tx, end_rx) = channel();

        let store = Store::new();
        let mount_manager = crate::fs::MountManager::new(store.clone());

        let tmp_dir = tempdir::TempDir::new("cultivate-test").unwrap();
        let tmp_dir_path = tmp_dir.path().to_path_buf();
        let tmp_dir_path2 = tmp_dir.path().to_path_buf();
        let handler = std::thread::spawn(move || {
            // Mount the vfs.
            mount_manager.mount(tmp_dir_path).unwrap();

            // Let the closure run.
            start_tx.send(()).unwrap();
            let _: () = end_rx.recv().unwrap();

            // Clean up the mount.
            drop(mount_manager);
            tmp_dir.close().unwrap()
        });

        // Run the closure after the filesystem is mounted.
        let _: () = start_rx.recv().unwrap();
        func(tmp_dir_path2, store);

        // Signal time to cleanup file system.
        end_tx.send(()).unwrap();

        // Wait for cleanup to finish.
        handler.join().unwrap();
    }

    #[test]
    fn read_empty_dir() {
        setup_mount(|mount_path, store| {
            let mut entries = fs::read_dir(mount_path)
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            assert_eq!(entries.len(), 0);
        });
    }
}
