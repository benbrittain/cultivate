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
    ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request, FUSE_ROOT_ID,
};
use tracing::info;

use crate::{
    mount_store::{self, DirectoryDescriptor, FileKind, Inode, InodeAttributes, MountStore},
    store::Store,
};

struct CultivateFS {
    store: Store,
    mount_store: MountStore,
}

impl CultivateFS {
    pub fn new(store: Store, mount_store: MountStore) -> Self {
        CultivateFS { store, mount_store }
    }

    fn get_inode(&self, inode: Inode) -> Result<InodeAttributes, libc::c_int> {
        if let Some(attr) = self.mount_store.get_inode(inode) {
            return Ok(attr.clone());
        }
        Err(libc::ENOENT)
    }

    //fn write_inode(&self, attrs: &InodeAttributes) {
    //    self.store.write_inode(attrs.clone())
    //}

    //fn write_directory_content(&self, inode: Inode, entries: DirectoryDescriptor) {
    //    self.store.write_directory_content(inode.clone(), entries)
    //}

    fn get_directory_content(&self, inode: Inode) -> Result<DirectoryDescriptor, libc::c_int> {
        info!("Get directory contents for {inode}");
        if let Some(attr) = self.mount_store.get_directory_content(inode) {
            return Ok(attr.clone());
        }
        Err(libc::ENOENT)
    }

    fn lookup_name(&self, parent: Inode, name: &OsStr) -> Result<InodeAttributes, c_int> {
        let entries = self.get_directory_content(parent)?;
        if let Some((inode, _)) = entries.get(name.as_bytes()) {
            self.get_inode(*inode)
        } else {
            Err(libc::ENOENT)
        }
    }
}

impl Filesystem for CultivateFS {
    fn lookup(&mut self, req: &Request, parent: Inode, name: &OsStr, reply: ReplyEntry) {
        dbg!(name);
        // TODO define actual length
        if name.len() > 40 as usize {
            reply.error(libc::ENAMETOOLONG);
            return;
        }

        match self.lookup_name(parent, name) {
            Ok(attrs) => reply.entry(&Duration::new(0, 0), &attrs.into(), 0),
            Err(error_code) => {
                dbg!(error_code);
                reply.error(error_code)
            }
        }
    }

    //fn init(
    //    &mut self,
    //    req: &Request,
    //    #[allow(unused_variables)] config: &mut KernelConfig,
    //) -> Result<(), libc::c_int> {
    //    if self.get_inode(FUSE_ROOT_ID).is_err() {
    //        let root = InodeAttributes::from_tree_id(FUSE_ROOT_ID, self.store.get_root_tree_id());
    //        self.write_inode(&root);
    //        let mut entries = BTreeMap::new();
    //        entries.insert(b".".to_vec(), (FUSE_ROOT_ID, FileKind::Directory));
    //        self.store.write_directory_content(FUSE_ROOT_ID, entries);
    //    }
    //    Ok(())
    //}

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

        // Fill the reply buffer as much as possible based upon the entries
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
        info!("Getting attributes for {inode}");
        match self.get_inode(inode) {
            Ok(attrs) => reply.attr(&Duration::new(0, 0), &attrs.into()),
            Err(error_code) => reply.error(error_code),
        }
    }
}

pub struct MountManager {
    store: Store,
    mounts: Vec<fuser::BackgroundSession>,
}

impl MountManager {
    pub fn new(store: Store) -> Self {
        MountManager {
            store,
            mounts: vec![],
        }
    }

    pub fn mount<P: Into<PathBuf> + std::fmt::Debug>(
        &mut self,
        mountpoint: P,
        mount_store: MountStore,
    ) -> Result<fuser::Notifier, Error> {
        let mountpoint = mountpoint.into();

        let mut options = vec![
            MountOption::FSName("cultivate".to_string()),
            MountOption::AutoUnmount,
            MountOption::NoDev,
            MountOption::Exec,
            MountOption::NoSuid,
        ];
        if mountpoint.is_dir() {
            let session = fuser::Session::new(
                CultivateFS::new(self.store.clone(), mount_store),
                &mountpoint,
                &options,
            )?;
            let notifier = session.notifier();
            let bg = session.spawn().unwrap();
            self.mounts.push(bg);
            Ok(notifier)
        } else {
            Err(anyhow!("No directory to mount filesystem at exists"))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, sync::mpsc::channel};

    use super::*;
    use crate::store::{Tree, TreeEntry};

    fn setup_mount(func: fn(PathBuf, Store, MountStore)) {
        let (start_tx, start_rx) = channel();
        let (end_tx, end_rx) = channel();

        let store = Store::new();
        let mount_store = MountStore::new();
        let mount_store2 = mount_store.clone();
        let mut mount_manager = crate::fs::MountManager::new(store.clone());

        let tmp_dir = tempdir::TempDir::new("cultivate-test").unwrap();
        let tmp_dir_path = tmp_dir.path().to_path_buf();
        let tmp_dir_path2 = tmp_dir.path().to_path_buf();
        let handler = std::thread::spawn(move || {
            // Mount the vfs.
            mount_manager.mount(tmp_dir_path, mount_store2).unwrap();

            // Let the closure run.
            start_tx.send(()).unwrap();
            // Don't unwrap, if the thread panics it'll hide
            // the error we want to see in the backtrace.
            let _ = end_rx.recv();

            // Clean up the mount.
            drop(mount_manager);
            tmp_dir.close().unwrap()
        });

        // Run the closure after the filesystem is mounted.
        let _: () = start_rx.recv().unwrap();
        func(tmp_dir_path2, store, mount_store);

        // Signal time to cleanup file system.
        end_tx.send(()).unwrap();

        // Wait for cleanup to finish.
        handler.join().unwrap();
    }

    #[test]
    fn read_empty_dir() {
        setup_mount(|mount_path, store, mount_store| {
            let tree_id = store.write_tree(Tree { entries: vec![] });
            mount_store.set_root_tree(&store, tree_id);

            let mut entries = fs::read_dir(mount_path)
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            assert_eq!(entries.len(), 0);
        });
    }

    #[test]
    fn read_simple_tree_from_dir() {
        setup_mount(|mount_path, store, mount_store| {
            let child_id = store.write_tree(Tree { entries: vec![] });
            let tree_id = store.write_tree(Tree {
                entries: vec![("test".to_string(), TreeEntry::TreeId(child_id))],
            });
            mount_store.set_root_tree(&store, tree_id);

            let mut entries = fs::read_dir(mount_path)
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            assert_eq!(entries.len(), 1);
        });
    }
}
