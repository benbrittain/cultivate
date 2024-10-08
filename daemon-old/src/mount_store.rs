use std::{
    collections::{BTreeMap, HashMap},
    sync::{atomic::AtomicU64, Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tracing::{error, info};

use crate::store::{Id, Store, TreeEntry};

const BLOCK_SIZE: u64 = 512;

pub type OperationId = [u8; 64];
pub type WorkspaceId = String;

/// Index Node Number
pub type Inode = u64;

pub type DirectoryDescriptor = BTreeMap<Vec<u8>, (Inode, FileKind)>;

#[derive(Clone, Debug)]
pub struct MountStore {
    store: Store,
    nodes: Arc<Mutex<HashMap<Inode, InodeAttributes>>>,
    directories: Arc<Mutex<HashMap<Inode, DirectoryDescriptor>>>,
    next_inode: Arc<AtomicU64>,

    op_id: Arc<Mutex<Option<OperationId>>>,
    workspace_id: Arc<Mutex<Option<WorkspaceId>>>,
    tree_id: Arc<Mutex<Id>>,
}

impl MountStore {
    pub fn new(store: Store) -> Self {
        let tree_id = store.get_empty_tree_id();
        MountStore {
            store,
            nodes: Arc::new(Mutex::new(HashMap::new())),
            directories: Arc::new(Mutex::new(HashMap::new())),
            next_inode: Arc::new(AtomicU64::new(1)),
            op_id: Arc::new(Mutex::new(None)),
            workspace_id: Arc::new(Mutex::new(None)),
            tree_id: Arc::new(Mutex::new(tree_id)),
        }
    }

    pub fn allocate_inode(&self) -> Inode {
        self.next_inode
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    pub fn get_tree_id(&self) -> Id {
        let tree_id = self.tree_id.lock().unwrap();
        *tree_id
    }

    pub fn get_op_id(&self) -> OperationId {
        let op_id = self.op_id.lock().unwrap();
        op_id.unwrap()
    }

    pub fn set_op_id(&self, op: OperationId) {
        let mut op_id = self.op_id.lock().unwrap();
        *op_id = Some(op);
    }

    pub fn get_workspace_id(&self) -> WorkspaceId {
        let workspace_id = self.workspace_id.lock().unwrap();
        workspace_id.clone().unwrap()
    }

    pub fn set_workspace_id(&self, op: WorkspaceId) {
        let mut workspace_id = self.workspace_id.lock().unwrap();
        *workspace_id = Some(op);
    }

    pub fn set_root_tree(&self, store: &Store, hash: Id) {
        // burn an inode
        let _ = self.allocate_inode();
        self.insert_tree(store, hash, 1)
    }

    pub fn insert_file(&self, store: &Store, hash: Id, _executable: bool, inode: Inode) {
        let file = store
            .get_file(hash)
            .expect("HashId must refer to a known file");
        let size = file.content.len();
        let mut attrs = InodeAttributes::new(inode, FileKind::File, size as u64);
        attrs.hash = Some(hash);
        self.set_inode(attrs);
    }

    pub fn insert_symlink(&self, store: &Store, hash: Id, inode: Inode) {
        let file = store
            .get_symlink(hash)
            .expect("HashId must refer to a known symlink");
        let size = file.target.len();
        let mut attrs = InodeAttributes::new(inode, FileKind::Symlink, size as u64);
        attrs.hash = Some(hash);
        self.set_inode(attrs);
    }

    pub fn insert_tree(&self, store: &Store, hash: Id, inode: Inode) {
        let tree = store
            .get_tree(hash)
            .expect("HashId must refer to a known tree");

        let attrs = InodeAttributes::new(inode, FileKind::Directory, 0);

        let mut entries = BTreeMap::new();
        entries.insert(b".".to_vec(), (inode, FileKind::Directory));

        info!("Inserting inode {inode} for {hash:?}");
        for (entry_name, entry) in tree.entries {
            let new_inode = self.allocate_inode();
            info!("Inserting entry {entry:?} new_inode={new_inode}");
            match entry {
                TreeEntry::File { id, executable } => {
                    self.insert_file(store, id, executable, new_inode);
                    entries.insert(entry_name.into_bytes(), (new_inode, FileKind::File));
                }
                TreeEntry::TreeId(id) => {
                    self.insert_tree(store, id, new_inode);
                    entries.insert(entry_name.into_bytes(), (new_inode, FileKind::Directory));
                }
                TreeEntry::SymlinkId(id) => {
                    self.insert_symlink(store, id, new_inode);
                    entries.insert(entry_name.into_bytes(), (new_inode, FileKind::Symlink));
                }
                _ => todo!(),
            }
        }
        self.set_inode(attrs);
        self.set_directory_content(inode, entries);
    }

    pub fn create_new_node(&self, kind: FileKind) -> InodeAttributes {
        let inode = self.allocate_inode();
        let attrs = InodeAttributes::new(inode, kind, 0);
        self.set_inode(attrs.clone());
        attrs
    }

    pub fn set_inode(&self, attrs: InodeAttributes) {
        let mut nodes = self.nodes.lock().unwrap();
        nodes.insert(attrs.inode, attrs);
    }

    pub fn set_directory_content(&self, inode: Inode, descriptor: DirectoryDescriptor) {
        let mut directories = self.directories.lock().unwrap();
        directories.insert(inode, descriptor);
    }

    pub fn get_directory_content(&self, inode: Inode) -> Option<DirectoryDescriptor> {
        let directories = self.directories.lock().unwrap();
        let dirs = directories.get(&inode).cloned();
        dirs
    }

    pub fn get_inode(&self, inode: Inode) -> Option<InodeAttributes> {
        let inode_store = self.nodes.lock().unwrap();
        inode_store.get(&inode).cloned()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct InodeAttributes {
    inode: Inode,
    hash: Option<Id>,
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

impl InodeAttributes {
    // TODO this should really be attached to the lifetime
    // of some data.
    pub fn inc_file_handle(&mut self) {
        let prior = self.open_file_handles;
        self.open_file_handles += 1;
        info!(
            "{} open file handles: {}->{}",
            self.inode, prior, self.open_file_handles
        );
    }

    pub fn dec_hardlink_count(&mut self) {
        self.hardlinks -= 1;
    }

    pub fn dec_file_handle(&mut self) {
        let prior = self.open_file_handles;
        if self.open_file_handles == 0 {
            error!("Tried to decrement open file handles beneath 0");
            return;
        }
        self.open_file_handles -= 1;
        info!(
            "{} open file handles: {}->{}",
            self.inode, prior, self.open_file_handles
        );
    }

    pub fn set_hash(&mut self, hash: Id) {
        self.hash = Some(hash)
    }

    pub fn get_hash(&self) -> Option<Id> {
        self.hash
    }

    pub fn get_inode(&self) -> Inode {
        self.inode
    }

    pub fn get_mode(&self) -> u16 {
        self.mode
    }

    pub fn get_size(&self) -> u64 {
        self.size
    }

    pub fn set_size(&mut self, size: u64) {
        self.size = size
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

    pub fn set_uid(&mut self, uid: u32) {
        self.uid = uid
    }

    pub fn set_gid(&mut self, gid: u32) {
        self.gid = gid
    }

    pub fn get_gid(&self) -> u32 {
        self.gid
    }

    pub fn get_kind(&self) -> FileKind {
        self.kind
    }

    pub fn update_last_modified(&mut self) {
        self.last_modified = time_now();
    }
    pub fn update_last_metadata_changed(&mut self) {
        self.last_metadata_changed = time_now();
    }

    pub fn new(inode: Inode, kind: FileKind, size: u64) -> InodeAttributes {
        assert!(
            (kind == FileKind::Directory) && (size == 0)
                || kind == FileKind::File
                || kind == FileKind::Symlink
        );
        let hardlinks = match kind {
            FileKind::File => 1,
            FileKind::Directory => 2,
            FileKind::Symlink => 1,
        };
        InodeAttributes {
            inode,
            hash: None,
            open_file_handles: 0,
            size,
            last_accessed: time_now(),
            last_modified: time_now(),
            last_metadata_changed: time_now(),
            kind,
            mode: 0o777,
            hardlinks,
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
fn system_time_from_time(secs: i64, nsecs: u32) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::new(secs as u64, nsecs)
    } else {
        UNIX_EPOCH - Duration::new((-secs) as u64, nsecs)
    }
}
