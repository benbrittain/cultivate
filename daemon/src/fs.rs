use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::atomic::AtomicU64,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Error;
use fuser::{
    Filesystem, KernelConfig, MountOption, ReplyAttr, ReplyData, ReplyDirectory,
    ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs, ReplyWrite, Request,
};

const BLOCK_SIZE: u64 = 512;

struct CultivateFS {
    data_dir: PathBuf,
    next_file_handle: AtomicU64,
}

type Inode = u64;

fn system_time_from_time(secs: i64, nsecs: u32) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::new(secs as u64, nsecs)
    } else {
        UNIX_EPOCH - Duration::new((-secs) as u64, nsecs)
    }
}

//#[derive(Serialize, Deserialize)]
struct InodeAttributes {
    pub inode: Inode,
    pub open_file_handles: u64, // Ref count of open file handles to this inode
    pub size: u64,
    pub last_accessed: (i64, u32),
    pub last_modified: (i64, u32),
    pub last_metadata_changed: (i64, u32),
    pub kind: FileKind,
    // Permissions and special mode bits
    pub mode: u16,
    pub hardlinks: u32,
    pub uid: u32,
    pub gid: u32,
    pub xattrs: BTreeMap<Vec<u8>, Vec<u8>>,
}

impl From<InodeAttributes> for fuser::FileAttr {
    fn from(attrs: InodeAttributes) -> Self {
        fuser::FileAttr {
            ino: attrs.inode,
            size: attrs.size,
            blocks: (attrs.size + BLOCK_SIZE - 1) / BLOCK_SIZE,
            atime: system_time_from_time(attrs.last_accessed.0, attrs.last_accessed.1),
            mtime: system_time_from_time(attrs.last_modified.0, attrs.last_modified.1),
            ctime: system_time_from_time(
                attrs.last_metadata_changed.0,
                attrs.last_metadata_changed.1,
            ),
            crtime: SystemTime::UNIX_EPOCH,
            kind: attrs.kind.into(),
            perm: attrs.mode,
            nlink: attrs.hardlinks,
            uid: attrs.uid,
            gid: attrs.gid,
            rdev: 0,
            blksize: BLOCK_SIZE as u32,
            flags: 0,
        }
    }
}

//#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
#[derive(Copy, Clone, PartialEq)]
enum FileKind {
    File,
    Directory,
    Symlink,
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
impl CultivateFS {
    pub fn new(data_dir: &Path) -> Self {
        CultivateFS {
            data_dir: data_dir.to_path_buf(),
            next_file_handle: AtomicU64::new(1),
        }
    }

    fn get_inode(&self, inode: Inode) -> Result<InodeAttributes, libc::c_int> {
        dbg!(inode);
        //let path = Path::new(&self.data_dir)
        //    .join("inodes")
        //    .join(inode.to_string());
        //if let Ok(file) = File::open(path) {
        //    Ok(bincode::deserialize_from(file).unwrap())
        //} else {
        Err(libc::ENOENT)
        //}
    }
}

pub fn mount(mountpoint: &Path, data_dir: &Path) -> Result<(), Error> {
    let options = vec![MountOption::FSName("cultivate".to_string())];
    fuser::mount2(CultivateFS::new(data_dir), mountpoint, &options)?;
    Ok(())
}

impl Filesystem for CultivateFS {
    fn lookup(&mut self, req: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        dbg!(req);
    }
    fn init(
        &mut self,
        req: &Request,
        #[allow(unused_variables)] config: &mut KernelConfig,
    ) -> Result<(), libc::c_int> {
        Ok(())
    }

    fn statfs(&mut self, _req: &Request, _ino: u64, reply: ReplyStatfs) {
        dbg!("statfs() implementation is a stub");
    }

    fn access(&mut self, req: &Request, inode: u64, mask: i32, reply: ReplyEmpty) {
        dbg!("access() called with {:?} {:?}", inode, mask);
    }
    fn readdir(
        &mut self,
        _req: &Request,
        inode: u64,
        _fh: u64,
        offset: i64,
        reply: ReplyDirectory,
    ) {
        dbg!("readdir() called with {:?}", inode);
    }

    fn opendir(&mut self, req: &Request, inode: u64, flags: i32, reply: ReplyOpen) {
        dbg!("opendir() called on {:?}", inode);
    }

    fn open(&mut self, req: &Request, inode: u64, flags: i32, reply: ReplyOpen) {
        dbg!("open() called for {:?}", inode);
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
        dbg!(
            "read() called on {:?} offset={:?} size={:?}",
            inode,
            offset,
            size
        );
        assert!(offset >= 0);
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
        dbg!("write() called with {:?} size={:?}", inode, data.len());
        assert!(offset >= 0);
    }

    fn forget(&mut self, _req: &Request, _ino: u64, _nlookup: u64) {}

    fn getattr(&mut self, _req: &Request, inode: u64, reply: ReplyAttr) {
        match self.get_inode(inode) {
            Ok(attrs) => reply.attr(&Duration::new(0, 0), &attrs.into()),
            Err(error_code) => reply.error(error_code),
        }
    }
}
