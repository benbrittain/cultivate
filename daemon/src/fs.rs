use std::{
    cmp::min,
    ffi::OsStr,
    io::{Cursor, Read, Write},
    os::unix::ffi::OsStrExt,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

use fuser::{
    Filesystem, KernelConfig, ReplyAttr, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry,
    ReplyOpen, ReplyStatfs, ReplyWrite, Request, TimeOrNow, FUSE_ROOT_ID,
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

pub struct CultivateFS {
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

    fn get_directory_content(&self, inode: Inode) -> Result<DirectoryDescriptor, libc::c_int> {
        info!("Get directory contents for {inode}");
        if let Some(attr) = self.mount_store.get_directory_content(inode) {
            return Ok(attr.clone());
        }
        Err(libc::ENOENT)
    }

    fn lookup_name(&self, parent: Inode, name: &OsStr) -> Result<InodeAttributes, libc::c_int> {
        info!("Lookup {name:?}, parent={parent}");
        let entries = self.get_directory_content(parent)?;
        if let Some((inode, _)) = entries.get(name.as_bytes()) {
            let inode = self.get_inode(*inode);
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

    fn insert_link(
        &self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        inode: u64,
        kind: FileKind,
    ) -> Result<(), libc::c_int> {
        if self.lookup_name(parent, name).is_ok() {
            return Err(libc::EEXIST);
        }

        let mut parent_attrs = self.get_inode(parent)?;

        if !check_access(
            parent_attrs.get_uid(),
            parent_attrs.get_gid(),
            parent_attrs.get_mode(),
            req.uid(),
            req.gid(),
            libc::W_OK,
        ) {
            return Err(libc::EACCES);
        }
        parent_attrs.update_last_modified();
        parent_attrs.update_last_metadata_changed();
        self.mount_store.set_inode(parent_attrs);

        let mut entries = self.get_directory_content(parent).unwrap();
        entries.insert(name.as_bytes().to_vec(), (inode, kind));
        self.mount_store.set_directory_content(parent, entries);

        Ok(())
    }
}

impl Filesystem for CultivateFS {
    fn lookup(&mut self, _req: &Request, parent: Inode, name: &OsStr, reply: ReplyEntry) {
        // TODO define actual length
        if name.len() > 140 as usize {
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
        _req: &Request,
        #[allow(unused_variables)] config: &mut KernelConfig,
    ) -> Result<(), libc::c_int> {
        if self.get_inode(FUSE_ROOT_ID).is_err() {
            self.mount_store
                .set_root_tree(&self.store, self.store.empty_tree_id)
        }
        Ok(())
    }

    fn setxattr(
        &mut self,
        _request: &Request<'_>,
        _inode: u64,
        _key: &OsStr,
        _value: &[u8],
        _flags: i32,
        _position: u32,
        _reply: ReplyEmpty,
    ) {
        todo!();
    }

    //fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
    //    warn!("statfs() implementation is a stub");
    //}

    fn access(&mut self, req: &Request, inode: u64, mask: i32, reply: ReplyEmpty) {
        info!("access() called with {:?} {:?}", inode, mask);
        // TODO access control
        reply.ok();
    }

    fn rmdir(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEmpty) {
        error!("rmdir() called with {:?} {:?}", parent, name);
        panic!();
    }

    fn rename(
        &mut self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        new_parent: u64,
        new_name: &OsStr,
        flags: u32,
        reply: ReplyEmpty,
    ) {
        let mut inode_attrs = match self.lookup_name(parent, name) {
            Ok(attrs) => attrs,
            Err(error_code) => {
                reply.error(error_code);
                return;
            }
        };

        let mut parent_attrs = match self.get_inode(parent) {
            Ok(attrs) => attrs,
            Err(error_code) => {
                reply.error(error_code);
                return;
            }
        };

        if !check_access(
            parent_attrs.get_uid(),
            parent_attrs.get_gid(),
            parent_attrs.get_mode(),
            req.uid(),
            req.gid(),
            libc::W_OK,
        ) {
            reply.error(libc::EACCES);
            return;
        }

        // "Sticky bit" handling
        // if parent_attrs.mode & libc::S_ISVTX as u16 != 0
        //     && req.uid() != 0
        //     && req.uid() != parent_attrs.uid
        //     && req.uid() != inode_attrs.uid
        // {
        //     reply.error(libc::EACCES);
        //     return;
        // }

        let mut new_parent_attrs = match self.get_inode(new_parent) {
            Ok(attrs) => attrs,
            Err(error_code) => {
                reply.error(error_code);
                return;
            }
        };

        if !check_access(
            new_parent_attrs.get_uid(),
            new_parent_attrs.get_gid(),
            new_parent_attrs.get_mode(),
            req.uid(),
            req.gid(),
            libc::W_OK,
        ) {
            reply.error(libc::EACCES);
            return;
        }

        // // "Sticky bit" handling in new_parent
        // if new_parent_attrs.mode & libc::S_ISVTX as u16 != 0 {
        //     if let Ok(existing_attrs) = self.lookup_name(new_parent, new_name) {
        //         if req.uid() != 0
        //             && req.uid() != new_parent_attrs.uid
        //             && req.uid() != existing_attrs.uid
        //         {
        //             reply.error(libc::EACCES);
        //             return;
        //         }
        //     }
        // }

        #[cfg(target_os = "linux")]
        if flags & libc::RENAME_EXCHANGE as u32 != 0 {
            todo!();
        }

        // Only overwrite an existing directory if it's empty
        if let Ok(new_name_attrs) = self.lookup_name(new_parent, new_name) {
            if new_name_attrs.get_kind() == FileKind::Directory
                && self
                    .get_directory_content(new_name_attrs.get_inode())
                    .unwrap()
                    .len()
                    > 2
            {
                reply.error(libc::ENOTEMPTY);
                return;
            }
        }

        // Only move an existing directory to a new parent, if we have write access to it,
        // because that will change the ".." link in it
        if inode_attrs.get_kind() == FileKind::Directory
            && parent != new_parent
            && !check_access(
                inode_attrs.get_uid(),
                inode_attrs.get_gid(),
                inode_attrs.get_mode(),
                req.uid(),
                req.gid(),
                libc::W_OK,
            )
        {
            reply.error(libc::EACCES);
            return;
        }

        // If target already exists decrement its hardlink count
        if let Ok(mut existing_inode_attrs) = self.lookup_name(new_parent, new_name) {
            let mut entries = self.get_directory_content(new_parent).unwrap();
            entries.remove(new_name.as_bytes());
            self.mount_store.set_directory_content(new_parent, entries);

            if existing_inode_attrs.get_kind() == FileKind::Directory {
                todo!();
                //existing_inode_attrs.hardlinks = 0;
            } else {
                existing_inode_attrs.dec_hardlink_count();
            }
            existing_inode_attrs.update_last_metadata_changed();
            self.mount_store.set_inode(existing_inode_attrs);
            warn!("not GCing Inode! FIX THIS!");
            //self.gc_inode(&existing_inode_attrs);
        }

        let mut entries = self.get_directory_content(parent).unwrap();
        entries.remove(name.as_bytes());
        self.mount_store.set_directory_content(parent, entries);

        let mut entries = self.get_directory_content(new_parent).unwrap();
        entries.insert(
            new_name.as_bytes().to_vec(),
            (inode_attrs.get_inode(), inode_attrs.get_kind()),
        );
        self.mount_store.set_directory_content(new_parent, entries);

        parent_attrs.update_last_modified();
        parent_attrs.update_last_metadata_changed();
        self.mount_store.set_inode(parent_attrs);
        new_parent_attrs.update_last_metadata_changed();
        new_parent_attrs.update_last_modified();
        self.mount_store.set_inode(new_parent_attrs);
        inode_attrs.update_last_metadata_changed();
        self.mount_store.set_inode(inode_attrs.clone());

        // change the .. to the new parent
        if inode_attrs.get_kind() == FileKind::Directory {
            let mut entries = self.get_directory_content(inode_attrs.get_inode()).unwrap();
            entries.insert(b"..".to_vec(), (new_parent, FileKind::Directory));
            self.mount_store
                .set_directory_content(inode_attrs.get_inode(), entries);
        }

        reply.ok();
    }

    fn readdir(
        &mut self,
        _req: &Request,
        inode: u64,
        _fh: u64,
        offset: i64,
        mut reply: ReplyDirectory,
    ) {
        assert!(offset >= 0);
        let entries = match self.get_directory_content(inode) {
            Ok(entries) => entries,
            Err(error_code) => {
                reply.error(error_code);
                return;
            }
        };
        info!("readdir() called with {:?} {entries:?}", inode);

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
        info!("opendir() called on {:?}", inode);
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
                if check_access(
                    attr.get_uid(),
                    attr.get_gid(),
                    attr.get_mode(),
                    req.uid(),
                    req.gid(),
                    access_mask,
                ) {
                    attr.inc_file_handle();
                    self.mount_store.set_inode(attr);
                    let open_flags = 0;
                    let fh = self.allocate_next_file_handle(read, write);
                    info!("file handle: {}", fh);
                    info!("file handle read: {}", self.check_file_handle_read(fh));
                    info!("file handle write: {}", self.check_file_handle_write(fh));
                    reply.opened(fh, open_flags);
                } else {
                    reply.error(libc::EACCES);
                }
                return;
            }
            Err(error_code) => reply.error(error_code),
        }
    }

    fn open(&mut self, req: &Request, inode: u64, flags: i32, reply: ReplyOpen) {
        info!("open() called for {:?}", inode);
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
                if check_access(
                    attr.get_uid(),
                    attr.get_gid(),
                    attr.get_mode(),
                    req.uid(),
                    req.gid(),
                    access_mask,
                ) {
                    attr.inc_file_handle();
                    self.mount_store.set_inode(attr);
                    let open_flags = 0;
                    let fh = self.allocate_next_file_handle(read, write);
                    info!("file handle: {}", fh);
                    info!("file handle read: {}", self.check_file_handle_read(fh));
                    info!("file handle write: {}", self.check_file_handle_write(fh));
                    reply.opened(fh, open_flags);
                } else {
                    reply.error(libc::EACCES);
                }
                return;
            }
            Err(error_code) => reply.error(error_code),
        }
    }

    fn setattr(
        &mut self,
        _req: &Request,
        inode: u64,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        _size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        _fh: Option<u64>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<u32>,
        reply: ReplyAttr,
    ) {
        let mut attrs = match self.get_inode(inode) {
            Ok(attrs) => attrs,
            Err(error_code) => {
                reply.error(error_code);
                return;
            }
        };
        warn!("Setattr not implemented");
        let attrs = self.get_inode(inode).unwrap();
        reply.attr(&Duration::new(0, 0), &attrs.into());
    }

    fn link(
        &mut self,
        _req: &Request,
        inode: u64,
        new_parent: u64,
        new_name: &OsStr,
        _reply: ReplyEntry,
    ) {
        info!(
            "link() called for {}, {}, {:?}",
            inode, new_parent, new_name
        );
        todo!()
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

    fn releasedir(
        &mut self,
        _req: &Request<'_>,
        inode: u64,
        _fh: u64,
        _flags: i32,
        reply: ReplyEmpty,
    ) {
        if let Ok(mut attrs) = self.get_inode(inode) {
            attrs.dec_file_handle();
            self.mount_store.set_inode(attrs);
        }
        reply.ok();
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
            attrs.dec_file_handle();
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
            warn!("attributes: {:#?}", attrs.clone());
            let mut file = match attrs.get_hash() {
                Some(hash) => files.get(&hash).expect("file to exist").clone(),
                None => crate::store::File::default(),
            };

            attrs.update_last_modified();
            attrs.update_last_metadata_changed();
            if data.len() + offset as usize > attrs.get_size() as usize {
                attrs.set_size((data.len() + offset as usize) as u64);
            }

            let mut content = Cursor::new(file.content);
            content.set_position(offset as u64);
            content.write_all(data).unwrap();
            file.content = content.into_inner();

            let hash = file.get_hash();
            files.insert(hash, file);
            // there is no GC mechanism right now
            attrs.set_hash(hash);

            self.mount_store.set_inode(attrs.clone());
            reply.written(data.len() as u32);
        } else {
            reply.error(libc::EBADF);
        }
    }

    fn readlink(&mut self, _req: &Request, inode: u64, reply: ReplyData) {
        info!("readlink() called on {:?}", inode);
        if let Some(attr) = self.mount_store.get_inode(inode) {
            let hash = attr.get_hash().unwrap();
            let symlink = self
                .store
                .get_symlink(hash)
                .expect("There should be a symlink in the store");
            let size = symlink.target.len();
            info!("readlink {symlink:?}");
            return reply.data(&symlink.target.as_bytes());
        }
        reply.error(libc::ENOENT);
    }

    fn symlink(
        &mut self,
        req: &Request,
        parent: u64,
        link_name: &OsStr,
        target: &Path,
        reply: ReplyEntry,
    ) {
        info!(
            "symlink() called with {:?} {:?} {:?}",
            parent, link_name, target
        );
        let mut parent_attrs = match self.get_inode(parent) {
            Ok(attrs) => attrs,
            Err(error_code) => {
                reply.error(error_code);
                return;
            }
        };

        if !check_access(
            parent_attrs.get_uid(),
            parent_attrs.get_gid(),
            parent_attrs.get_mode(),
            req.uid(),
            req.gid(),
            libc::W_OK,
        ) {
            reply.error(libc::EACCES);
            return;
        }
        parent_attrs.update_last_modified();
        parent_attrs.update_last_metadata_changed();
        self.mount_store.set_inode(parent_attrs.clone());

        let mut attrs = self.mount_store.create_new_node(FileKind::Symlink);
        attrs.set_uid(req.uid());
        attrs.set_gid(creation_gid(&parent_attrs, req.gid()));
        attrs.set_size(target.as_os_str().as_bytes().len() as u64);

        if let Err(error_code) =
            self.insert_link(req, parent, link_name, attrs.get_inode(), FileKind::Symlink)
        {
            reply.error(error_code);
            return;
        }

        let mut symlinks = self.store.symlinks.lock().unwrap();
        let mut symlink = crate::store::Symlink::default();
        symlink.target = target.to_str().unwrap().to_string();
        let hash = symlink.get_hash();
        symlinks.insert(hash, symlink);
        attrs.set_hash(hash);
        self.mount_store.set_inode(attrs.clone());

        reply.entry(&Duration::new(0, 0), &attrs.into(), 0);
    }

    //fn create(
    //    &mut self,
    //    req: &Request,
    //    parent: u64,
    //    name: &OsStr,
    //    mut mode: u32,
    //    _umask: u32,
    //    flags: i32,
    //    reply: ReplyCreate,
    //) {
    //    warn!("create() called with {:?} {:?}", parent, name);
    //    if self.lookup_name(parent, name).is_ok() {
    //        reply.error(libc::EEXIST);
    //        return;
    //    }
    //    todo!()
    //}

    fn mkdir(
        &mut self,
        req: &Request,
        parent: u64,
        name: &OsStr,
        mut mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        info!("mkdir() called with {:?} {:?} {:o}", parent, name, mode);
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

        if !check_access(
            parent_attrs.get_uid(),
            parent_attrs.get_gid(),
            parent_attrs.get_mode(),
            req.uid(),
            req.gid(),
            libc::W_OK,
        ) {
            reply.error(libc::EACCES);
            return;
        }
        parent_attrs.update_last_modified();
        parent_attrs.update_last_metadata_changed();
        self.mount_store.set_inode(parent_attrs.clone());

        if req.uid() != 0 {
            mode &= !(libc::S_ISUID | libc::S_ISGID) as u32;
        }
        if parent_attrs.get_mode() & libc::S_ISGID as u16 != 0 {
            mode |= libc::S_ISGID as u32;
        }

        let mut attrs = self.mount_store.create_new_node(FileKind::Directory);
        // should this go in create new node? requires the Request
        attrs.set_uid(req.uid());
        attrs.set_gid(creation_gid(&parent_attrs, req.gid()));
        self.mount_store.set_inode(attrs.clone());

        let mut entries = self.get_directory_content(parent).unwrap();
        entries.insert(
            name.as_bytes().to_vec(),
            (attrs.get_inode(), attrs.get_kind()),
        );
        self.mount_store.set_directory_content(parent, entries);

        // create current dir
        let mut entries = std::collections::BTreeMap::new();
        entries.insert(b".".to_vec(), (attrs.get_inode(), FileKind::Directory));
        entries.insert(b"..".to_vec(), (parent, FileKind::Directory));
        self.mount_store
            .set_directory_content(attrs.get_inode(), entries);

        // update parent dir
        let mut entries = self.get_directory_content(parent).unwrap();
        entries.insert(
            name.as_bytes().to_vec(),
            (attrs.get_inode(), FileKind::Directory),
        );
        self.mount_store.set_directory_content(parent, entries);

        reply.entry(&Duration::new(0, 0), &attrs.into(), 0);
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

        parent_attrs.update_last_modified();
        parent_attrs.update_last_metadata_changed();
        self.mount_store.set_inode(parent_attrs.clone());

        if req.uid() != 0 {
            mode &= !(libc::S_ISUID | libc::S_ISGID) as u32;
        }

        let mut attrs = self.mount_store.create_new_node(as_file_kind(mode));
        // should this go in create new node? requires the Request
        attrs.set_uid(req.uid());
        attrs.set_gid(creation_gid(&parent_attrs, req.gid()));
        self.mount_store.set_inode(attrs.clone());

        let mut entries = self.get_directory_content(parent).unwrap();
        entries.insert(
            name.as_bytes().to_vec(),
            (attrs.get_inode(), attrs.get_kind()),
        );
        self.mount_store.set_directory_content(parent, entries);

        assert!(as_file_kind(mode) != FileKind::Directory);
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

fn creation_gid(parent: &InodeAttributes, gid: u32) -> u32 {
    if parent.get_mode() & libc::S_ISGID as u16 != 0 {
        return parent.get_gid();
    }

    gid
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

pub fn check_access(
    file_uid: u32,
    file_gid: u32,
    file_mode: u16,
    uid: u32,
    gid: u32,
    mut access_mask: i32,
) -> bool {
    // F_OK tests for existence of file
    if access_mask == libc::F_OK {
        return true;
    }
    let file_mode = i32::from(file_mode);

    // root is allowed to read & write anything
    if uid == 0 {
        // root only allowed to exec if one of the X bits is set
        access_mask &= libc::X_OK;
        access_mask -= access_mask & (file_mode >> 6);
        access_mask -= access_mask & (file_mode >> 3);
        access_mask -= access_mask & file_mode;
        return access_mask == 0;
    }

    if uid == file_uid {
        access_mask -= access_mask & (file_mode >> 6);
    } else if gid == file_gid {
        access_mask -= access_mask & (file_mode >> 3);
    } else {
        access_mask -= access_mask & file_mode;
    }

    return access_mask == 0;
}

#[cfg(test)]
mod tests {
    use std::{fs, future::Future, io::Write, path::PathBuf, sync::mpsc::channel};

    use tracing_test::traced_test;

    use super::*;
    use crate::store::{File, Tree, TreeEntry};

    async fn setup_mount<F: Fn(PathBuf, Store, MountStore) -> Fut, Fut: Future<Output = ()>>(
        func: F,
    ) {
        let (start_tx, start_rx) = channel();
        let (end_tx, end_rx) = channel();

        let store = Store::new();
        let repo_manager = crate::repo_manager::RepoManager::new(store.clone());
        let repo_manager2 = repo_manager.clone();

        let tmp_dir = tempdir::TempDir::new("cultivate-test").unwrap();
        let tmp_dir_path = tmp_dir.path().to_path_buf();
        let tmp_dir_path2 = tmp_dir.path().to_path_buf();
        let tmp_dir_path3 = tmp_dir.path().to_path_buf();
        let handler = std::thread::spawn(move || {
            // Mount the vfs.
            repo_manager2.initialize_repo(&tmp_dir_path);

            // Let the closure run.
            start_tx.send(()).unwrap();
            // Don't unwrap, if the thread panics it'll hide
            // the error we want to see in the backtrace.
            let _ = end_rx.recv();

            // Clean up the mount.
            repo_manager2.deinit_repo(&tmp_dir_path);
            tmp_dir.close().unwrap()
        });

        // Run the closure after the filesystem is mounted.
        let _: () = start_rx.recv().unwrap();
        let mount_store = repo_manager.get(tmp_dir_path3.to_str().unwrap()).unwrap();
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
                file.write_all(b"The Last Yak").unwrap();
                file.flush().unwrap();
            }
            {
                let mut file = std::fs::File::open(mount_path).unwrap();
                let mut content = vec![];
                file.read_to_end(&mut content).unwrap();
                assert_eq!(content, b"The Last Yak")
            }
        })
        .await
    }

    #[tokio::test]
    #[traced_test]
    async fn write_symlink() {
        setup_mount(|mut mount_path, store, mount_store| async move {
            // Empty tree
            let tree_id = store.write_tree(Tree { entries: vec![] }).await;
            mount_store.set_root_tree(&store, tree_id);
            let mut src = mount_path.clone();
            src.push("a.txt");
            let mut file = std::fs::File::create(src.clone()).unwrap();
            file.write_all(b"The Last Yak").unwrap();
            file.flush().unwrap();
            let mut target = mount_path.clone();
            target.push("b.txt");
            std::os::unix::fs::symlink(src.clone(), target.clone()).unwrap();
            let path = fs::read_link(target).unwrap();
            assert_eq!(path, src)
        })
        .await
    }

    #[tokio::test]
    #[traced_test]
    async fn append_to_file_in_tree() {
        setup_mount(|mut mount_path, store, mount_store| async move {
            // Empty tree
            let tree_id = store.write_tree(Tree { entries: vec![] }).await;
            mount_store.set_root_tree(&store, tree_id);
            mount_path.push("file1");
            {
                let mut file = std::fs::File::create(mount_path.clone()).unwrap();
                file.write_all(b"The Last Yak ").unwrap();
                file.flush().unwrap();
            }
            {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .write(true)
                    .open(mount_path.clone())
                    .unwrap();
                file.write_all(b"to be shaved").unwrap();
                file.flush().unwrap();
            }
            {
                let mut file = std::fs::File::open(mount_path).unwrap();
                let mut content = vec![];
                file.read_to_end(&mut content).unwrap();
                assert_eq!(content, b"The Last Yak to be shaved")
            }
        })
        .await
    }
}
