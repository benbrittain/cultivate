use std::{
    cmp::min,
    ffi::OsStr,
    io::{Cursor, Read, Write},
    os::unix::ffi::OsStrExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use anyhow::{anyhow, Error};
use fuser::{
    Filesystem, KernelConfig, MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty,
    ReplyEntry, ReplyOpen, ReplyWrite, Request, FUSE_ROOT_ID,
};
use tracing::{error, info, warn};

use crate::{
    mount_store::{DirectoryDescriptor, FileKind, Inode, InodeAttributes, MountStore},
    store::Store,
};

// Top two file handle bits are used to store permissions
// Note: This isn't safe, since the client can modify those bits. However, this implementation
// is just a toy
const FILE_HANDLE_READ_BIT: u64 = 1 << 63;
const FILE_HANDLE_WRITE_BIT: u64 = 1 << 62;
const FMODE_EXEC: i32 = 0x20;

struct CultivateFS {
    store: Store,
    mount_store: MountStore,
    next_file_handle: AtomicU64,
}

impl CultivateFS {
    pub fn new(store: Store, mount_store: MountStore) -> Self {
        CultivateFS {
            store,
            mount_store,
            next_file_handle: AtomicU64::new(1),
        }
    }

    fn get_inode(&self, inode: Inode) -> Result<InodeAttributes, libc::c_int> {
        if let Some(attr) = self.mount_store.get_inode(inode) {
            return Ok(attr.clone());
        }
        Err(libc::ENOENT)
    }

    //fn write_inode(&self, attrs: &InodeAttributes) {
    //    let new_inode = self.mount_store.allocate_inode();
    //    attrs.inode = new_inode;
    //    //self.mount_store.set_inode(attrs)
    //    //if let Some(attr) = self.mount_store.get_inode(inode) {
    //    //    return Ok(attr.clone());
    //    //}
    //    //Err(libc::ENOENT)
    //}

    //fn write_inode(&self, attrs: &InodeAttributes) {
    //    self.store.write_inode(attrs.clone())
    //}

    //fn write_directory_content(&self, inode: Inode, entries: DirectoryDescriptor) {
    //    self.store.write_directory_content(inode.clone(), entries)
    //}

    fn get_directory_content(&self, inode: Inode) -> Result<DirectoryDescriptor, libc::c_int> {
        info!("Get directory contents for {inode}");
        if let Some(attr) = self.mount_store.get_directory_content(inode) {
            info!("attr: {attr:#?}");
            return Ok(attr.clone());
        }
        Err(libc::ENOENT)
    }

    fn lookup_name(&self, parent: Inode, name: &OsStr) -> Result<InodeAttributes, libc::c_int> {
        info!("Lookup {name:?}, parent={parent}");
        let entries = self.get_directory_content(parent)?;
        if let Some((inode, _)) = entries.get(name.as_bytes()) {
            let inode = self.get_inode(*inode);
            info!("found: {inode:?}");
            inode
        } else {
            Err(libc::ENOENT)
        }
    }

    fn allocate_next_file_handle(&self, read: bool, write: bool) -> u64 {
        let mut fh = self.next_file_handle.fetch_add(1, Ordering::SeqCst);
        // Assert that we haven't run out of file handles
        assert!(fh < FILE_HANDLE_READ_BIT.min(FILE_HANDLE_WRITE_BIT));
        if read {
            fh |= FILE_HANDLE_READ_BIT;
        }
        if write {
            fh |= FILE_HANDLE_WRITE_BIT;
        }

        fh
    }

    fn check_file_handle_read(&self, file_handle: u64) -> bool {
        (file_handle & FILE_HANDLE_READ_BIT) != 0
    }

    fn check_file_handle_write(&self, file_handle: u64) -> bool {
        (file_handle & FILE_HANDLE_WRITE_BIT) != 0
    }
}

impl Filesystem for CultivateFS {
    fn lookup(&mut self, req: &Request, parent: Inode, name: &OsStr, reply: ReplyEntry) {
        info!("Lookup {name:?} parent={parent}");
        // TODO define actual length
        if name.len() > 40 as usize {
            reply.error(libc::ENAMETOOLONG);
            return;
        }

        match self.lookup_name(parent, name) {
            Ok(attrs) => reply.entry(&Duration::new(0, 0), &attrs.into(), 0),
            Err(error_code) => {
                warn!("Lookup for {name:?} failed with {error_code}");
                reply.error(error_code)
            }
        }
    }

    fn init(
        &mut self,
        req: &Request,
        #[allow(unused_variables)] config: &mut KernelConfig,
    ) -> Result<(), libc::c_int> {
        if self.get_inode(FUSE_ROOT_ID).is_err() {
            self.mount_store
                .set_root_tree(&self.store, self.store.empty_tree_id)
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
        info!("readdir() called with {:?}", inode);
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

    fn opendir(&mut self, req: &Request, inode: u64, flags: i32, reply: ReplyOpen) {
        error!("opendir() called on {:?}", inode);
        let (access_mask, read, write) = match flags & libc::O_ACCMODE {
            libc::O_RDONLY => {
                // Behavior is undefined, but most filesystems return EACCES
                if flags & libc::O_TRUNC != 0 {
                    reply.error(libc::EACCES);
                    return;
                }
                (libc::R_OK, true, false)
            }
            libc::O_WRONLY => (libc::W_OK, false, true),
            libc::O_RDWR => (libc::R_OK | libc::W_OK, true, true),
            // Exactly one access mode flag must be specified
            _ => {
                reply.error(libc::EINVAL);
                return;
            }
        };
        match self.get_inode(inode) {
            Ok(mut attr) => {
                //if check_access(
                //    attr.uid,
                //    attr.gid,
                //    attr.mode,
                //    req.uid(),
                //    req.gid(),
                //    access_mask,
                //) {
                attr.open_file_handles += 1;
                self.mount_store.set_inode(attr);
                let open_flags = 0;
                reply.opened(self.allocate_next_file_handle(read, write), open_flags);
                //} else {
                //    reply.error(libc::EACCES);
                //}
                return;
            }
            Err(error_code) => reply.error(error_code),
        }
    }

    fn open(&mut self, req: &Request, inode: u64, flags: i32, reply: ReplyOpen) {
        let (access_mask, read, write) = match flags & libc::O_ACCMODE {
            libc::O_RDONLY => {
                // Behavior is undefined, but most filesystems return EACCES
                if flags & libc::O_TRUNC != 0 {
                    reply.error(libc::EACCES);
                    return;
                }
                if flags & FMODE_EXEC != 0 {
                    // Open is from internal exec syscall
                    (libc::X_OK, true, false)
                } else {
                    (libc::R_OK, true, false)
                }
            }
            libc::O_WRONLY => (libc::W_OK, false, true),
            libc::O_RDWR => (libc::R_OK | libc::W_OK, true, true),
            // Exactly one access mode flag must be specified
            _ => {
                reply.error(libc::EINVAL);
                return;
            }
        };

        match self.get_inode(inode) {
            Ok(mut attr) => {
                //if check_access(
                //    attr.uid,
                //    attr.gid,
                //    attr.mode,
                //    req.uid(),
                //    req.gid(),
                //    access_mask,
                //) {
                attr.open_file_handles += 1;
                self.mount_store.set_inode(attr);
                let open_flags = 0;
                reply.opened(self.allocate_next_file_handle(read, write), open_flags);
                //} else {
                //    reply.error(libc::EACCES);
                //}
                return;
            }
            Err(error_code) => reply.error(error_code),
        }
    }

    fn read(
        &mut self,
        _req: &Request,
        inode: u64,
        fh: u64,
        offset: i64,
        size: u32,
        _flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyData,
    ) {
        info!(
            "read() called on {:?} offset={:?} size={:?}",
            inode, offset, size
        );
        assert!(offset >= 0);
        if !self.check_file_handle_read(fh) {
            reply.error(libc::EACCES);
            return;
        }

        let files = self.store.files.lock().unwrap();
        if let Some(node) = self.mount_store.get_inode(inode) {
            let hash = node.get_hash().expect("node backed by file object");
            let raw_file = files.get(&hash).expect("file to exist");
            let mut file = Cursor::new(raw_file.content.clone());

            let file_size = raw_file.content.len() as u64;
            // Could underflow if file length is less than local_start
            let read_size = min(size, file_size.saturating_sub(offset as u64) as u32);

            let mut buffer = vec![0; read_size as usize];
            file.read_exact(&mut buffer[offset as usize..]).unwrap();
            reply.data(&buffer);
        } else {
            reply.error(libc::ENOENT);
        }
    }

    fn release(
        &mut self,
        _req: &Request<'_>,
        inode: u64,
        _fh: u64,
        _flags: i32,
        _lock_owner: Option<u64>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        if let Ok(mut attrs) = self.get_inode(inode) {
            attrs.open_file_handles -= 1;
            self.mount_store.set_inode(attrs);
        }
        reply.ok();
    }

    fn write(
        &mut self,
        _req: &Request,
        inode: u64,
        fh: u64,
        offset: i64,
        data: &[u8],
        _write_flags: u32,
        #[allow(unused_variables)] flags: i32,
        _lock_owner: Option<u64>,
        reply: ReplyWrite,
    ) {
        info!("write() called with {:?} size={:?}", inode, data.len());
        assert!(offset >= 0);
        if !self.check_file_handle_write(fh) {
            reply.error(libc::EACCES);
            return;
        }

        // this is all a kludgy mess. Need to implement an overlay
        // and a backend filestore
        let mut files = self.store.files.lock().unwrap();
        if let Some(mut attrs) = self.mount_store.get_inode(inode) {
            let mut file = match attrs.get_hash() {
                Some(hash) => files.get(&hash).expect("file to exist").clone(),
                None => crate::store::File::default(),
            };

            let mut content = Cursor::new(file.content);
            content.set_position(offset as u64);
            content.write_all(data).unwrap();
            file.content = content.into_inner();

            let hash = file.get_hash();
            files.insert(hash, file);
            attrs.set_hash(hash);
            info!("Updated inode={inode} to {attrs:?}");

            self.mount_store.set_inode(attrs);
            reply.written(data.len() as u32);
        } else {
            reply.error(libc::EBADF);
        }
    }

    fn mknod(
        &mut self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        mut mode: u32,
        _umask: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        let file_type = mode & libc::S_IFMT as u32;
        info!("mknod() called for {:?} mode={}", name, mode);

        if file_type != libc::S_IFREG as u32
            && file_type != libc::S_IFLNK as u32
            && file_type != libc::S_IFDIR as u32
        {
            warn!("mknod() implementation is incomplete. Only supports regular files, symlinks, and directories. Got {:o}", mode);
            reply.error(libc::ENOSYS);
            return;
        }

        if self.lookup_name(parent, name).is_ok() {
            reply.error(libc::EEXIST);
            return;
        }

        let mut parent_attrs = match self.get_inode(parent) {
            Ok(attrs) => attrs,
            Err(error_code) => {
                reply.error(error_code);
                return;
            }
        };

        // TODO access control

        // TODO last changed
        //parent_attrs.last_modified = time_now();
        //parent_attrs.last_metadata_changed = time_now();
        //self.write_inode(&parent_attrs);

        if req.uid() != 0 {
            mode &= !(libc::S_ISUID | libc::S_ISGID) as u32;
        }

        let attrs = self.mount_store.create_new_node(as_file_kind(mode));

        let mut entries = self.get_directory_content(parent).unwrap();
        entries.insert(
            name.as_bytes().to_vec(),
            (attrs.get_inode(), attrs.get_kind()),
        );
        self.mount_store.set_directory_content(parent, entries);

        //if as_file_kind(mode) == FileKind::Directory {
        //    let mut entries = BTreeMap::new();
        //    entries.insert(b".".to_vec(), (inode, FileKind::Directory));
        //    entries.insert(b"..".to_vec(), (parent, FileKind::Directory));
        //    self.write_directory_content(inode, entries);
        //}

        //let mut entries = self.get_directory_content(parent).unwrap();
        //entries.insert(name.as_bytes().to_vec(), (inode, attrs.kind));
        //self.write_directory_content(parent, entries);

        // TODO: implement flags
        reply.entry(&Duration::new(0, 0), &attrs.into(), 0);
    }

    //fn forget(&mut self, _req: &Request, _ino: u64, _nlookup: u64) {}

    fn getattr(&mut self, _req: &Request, inode: u64, reply: ReplyAttr) {
        info!("Getting attributes for {inode}");
        match self.get_inode(inode) {
            Ok(attrs) => reply.attr(&Duration::new(0, 0), &attrs.into()),
            Err(error_code) => reply.error(error_code),
        }
    }
}

fn as_file_kind(mut mode: u32) -> FileKind {
    mode &= libc::S_IFMT as u32;

    if mode == libc::S_IFREG as u32 {
        return FileKind::File;
    } else if mode == libc::S_IFLNK as u32 {
        return FileKind::Symlink;
    } else if mode == libc::S_IFDIR as u32 {
        return FileKind::Directory;
    } else {
        unimplemented!("{}", mode);
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

        let options = vec![
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
    use std::{fs, future::Future, io::Write, sync::mpsc::channel};

    use tracing_test::traced_test;

    use super::*;
    use crate::store::{File, Tree, TreeEntry};

    async fn setup_mount<F: Fn(PathBuf, Store, MountStore) -> Fut, Fut: Future<Output = ()>>(
        func: F,
    ) {
        let (start_tx, start_rx) = channel();
        let (end_tx, end_rx) = channel();

        let store = Store::new();
        let mount_store = MountStore::new(store.clone());
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
        func(tmp_dir_path2, store, mount_store).await;

        // Signal time to cleanup file system.
        end_tx.send(()).unwrap();

        // Wait for cleanup to finish.
        handler.join().unwrap();
    }

    #[tokio::test]
    #[traced_test]
    async fn read_empty_dir() {
        setup_mount(|mount_path, store, mount_store| async move {
            let tree_id = store.write_tree(Tree { entries: vec![] }).await;
            mount_store.set_root_tree(&store, tree_id);

            let entries = fs::read_dir(mount_path)
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            assert_eq!(entries.len(), 0);
        })
        .await;
    }

    #[tokio::test]
    #[traced_test]
    async fn read_single_file() {
        setup_mount(|mount_path, store, mount_store| async move {
            let file_id = store
                .write_file(File {
                    content: b"the last yak".to_vec(),
                })
                .await;

            let tree_id = store
                .write_tree(Tree {
                    entries: vec![(
                        "file_to_read".to_string(),
                        TreeEntry::File {
                            id: file_id,
                            executable: false,
                        },
                    )],
                })
                .await;
            mount_store.set_root_tree(&store, tree_id);
            let mut fin = mount_path.clone();
            fin.push("file_to_read");

            let file_content: String = fs::read_to_string(fin).unwrap().parse().unwrap();
            assert_eq!(file_content, "the last yak");
        })
        .await;
    }

    #[tokio::test]
    #[traced_test]
    async fn read_simple_tree_from_dir() {
        setup_mount(|mount_path, store, mount_store| async move {
            let child_id = store.write_tree(Tree { entries: vec![] }).await;
            let tree_id = store
                .write_tree(Tree {
                    entries: vec![("test".to_string(), TreeEntry::TreeId(child_id))],
                })
                .await;
            mount_store.set_root_tree(&store, tree_id);

            let entries = fs::read_dir(mount_path)
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            assert_eq!(entries.len(), 1);
        })
        .await;
    }

    #[tokio::test]
    #[traced_test]
    async fn read_simple_tree_from_dir_with_file() {
        setup_mount(|mount_path, store, mount_store| async move {
            let child_id = store.write_tree(Tree { entries: vec![] }).await;
            let file_id = store.write_file(File { content: vec![] }).await;
            let tree_id = store
                .write_tree(Tree {
                    entries: vec![
                        ("test_dir".to_string(), TreeEntry::TreeId(child_id)),
                        (
                            "test_file".to_string(),
                            TreeEntry::File {
                                id: file_id,
                                executable: false,
                            },
                        ),
                    ],
                })
                .await;
            mount_store.set_root_tree(&store, tree_id);

            let entries = fs::read_dir(mount_path)
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            assert_eq!(entries.len(), 2);
        })
        .await;
    }

    #[tokio::test]
    #[traced_test]
    async fn read_nested_simple_tree() {
        setup_mount(|mount_path, store, mount_store| async move {
            let file_id = store
                .write_file(File {
                    content: b"hello\n".to_vec(),
                })
                .await;
            let child_id = store
                .write_tree(Tree {
                    entries: vec![
                        (
                            "test_file".to_string(),
                            TreeEntry::File {
                                id: file_id,
                                executable: false,
                            },
                        ),
                        (
                            "test_file2".to_string(),
                            TreeEntry::File {
                                id: file_id,
                                executable: false,
                            },
                        ),
                    ],
                })
                .await;
            let tree_id = store
                .write_tree(Tree {
                    entries: vec![("test_dir".to_string(), TreeEntry::TreeId(child_id))],
                })
                .await;
            mount_store.set_root_tree(&store, tree_id);

            let entries = fs::read_dir(mount_path.clone())
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            assert_eq!(entries.len(), 1);

            let mut nested_path = mount_path.clone();
            nested_path.push("test_dir");
            let entries = fs::read_dir(nested_path)
                .unwrap()
                .map(|res| res.map(|e| e.path()))
                .collect::<Result<Vec<_>, std::io::Error>>()
                .unwrap();
            assert_eq!(entries.len(), 2);
        })
        .await;
    }

    #[tokio::test]
    #[traced_test]
    async fn write_file_to_tree() {
        setup_mount(|mut mount_path, store, mount_store| async move {
            // Empty tree
            let tree_id = store.write_tree(Tree { entries: vec![] }).await;
            mount_store.set_root_tree(&store, tree_id);
            mount_path.push("file1");
            {
                let mut file = std::fs::File::create(mount_path.clone()).unwrap();
                file.write_all(b"Hello, world!").unwrap();
                file.flush().unwrap();
            }
            {
                let mut file = std::fs::File::open(mount_path).unwrap();
                let mut content = String::new();
                file.read_to_string(&mut content).unwrap();
            }
        })
        .await
    }
}
