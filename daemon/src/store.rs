use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use prost::Message;
use proto::backend::{Commit, File};

use crate::content_hash::{blake3, ContentHash};

pub type Id = Vec<u8>;

#[derive(Clone, Debug)]
pub enum TreeEntry {
    File { id: Id, executable: bool },
    TreeId(Id),
    SymlinkId(Id),
    ConflictId(Id),
}

impl ContentHash for TreeEntry {
    fn update(&self, state: &mut blake3::Hasher) {
        match self {
            TreeEntry::File { id, executable } => {
                state.update(&[b'0']);
                ContentHash::update(id, state);
                ContentHash::update(executable, state);
            }
            TreeEntry::TreeId(tree_id) => {
                state.update(&[b'1']);
                ContentHash::update(tree_id, state);
            }
            _ => todo!(),
        }
    }
}

impl TreeEntry {
    pub fn as_proto(&self) -> proto::backend::TreeValue {
        let mut proto = proto::backend::TreeValue::default();
        proto.value = Some(match self {
            TreeEntry::File { id, executable } => {
                let mut proto_entry = proto::backend::tree_value::File::default();
                proto_entry.id = id.clone();
                proto_entry.executable = *executable;
                proto::backend::tree_value::Value::File(proto_entry)
            }
            _ => todo!(),
        });
        proto
    }
}

impl From<proto::backend::TreeValue> for TreeEntry {
    fn from(proto: proto::backend::TreeValue) -> Self {
        let value: proto::backend::tree_value::Value = proto.value.unwrap();
        use proto::backend::tree_value::Value::*;
        match value {
            TreeId(id) => TreeEntry::TreeId(id),
            SymlinkId(id) => TreeEntry::SymlinkId(id),
            ConflictId(id) => TreeEntry::ConflictId(id),
            File(file) => TreeEntry::File {
                id: file.id,
                executable: file.executable,
            },
        }
    }
}

content_hash! {
    #[derive(Clone, Debug, Default)]
    pub struct Tree {
        entries: Vec<(String, TreeEntry)>
    }
}

impl From<proto::backend::Tree> for Tree {
    fn from(proto: proto::backend::Tree) -> Self {
        let mut tree = Tree::default();
        for proto_entry in proto.entries {
            let proto_val = proto_entry.value.unwrap();
            let entry = proto_val.into();
            tree.entries.push((proto_entry.name, entry));
        }
        tree
    }
}

impl Tree {
    pub fn get_hash(&self) -> Id {
        blake3(self).as_bytes().to_vec()
    }

    pub fn as_proto(&self) -> proto::backend::Tree {
        let mut proto = proto::backend::Tree::default();
        for entry in &self.entries {
            let mut proto_entry = proto::backend::tree::Entry::default();
            proto_entry.name = entry.0.clone();
            proto_entry.value = Some(entry.1.as_proto());
            proto.entries.push(proto_entry);
        }
        dbg!("pyr: {:?}", &proto);
        proto
    }
}

/// Index Node Number
pub type Inode = u64;

pub type DirectoryDescriptor = BTreeMap<Vec<u8>, (Inode, FileKind)>;

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

#[derive(Clone, Debug)]
pub struct Store {
    // Maybe refactor these out? This is more of a TreeStore
    pub commits: Arc<Mutex<HashMap<Id, Commit>>>,
    pub files: Arc<Mutex<HashMap<Id, File>>>,

    pub trees: Arc<Mutex<HashMap<Id, Tree>>>,
    inode_store: Arc<Mutex<HashMap<Inode, InodeAttributes>>>,
    root_tree: Id,
    pub empty_tree_id: Id,
}

impl Store {
    pub fn new() -> Self {
        let commits = Arc::new(Mutex::new(HashMap::new()));
        let inode_store = Arc::new(Mutex::new(HashMap::new()));
        let files = Arc::new(Mutex::new(HashMap::new()));
        let (empty_tree_id, trees) = {
            let mut trees = HashMap::new();
            let tree = Tree::default();
            let empty_tree_id: Id = tree.get_hash();
            trees.insert(empty_tree_id.clone(), tree);
            (empty_tree_id, Arc::new(Mutex::new(trees)))
        };
        // The default tree is the empty tree.
        let root_tree = empty_tree_id.clone();
        Store {
            commits,
            inode_store,
            trees,
            files,
            empty_tree_id,
            root_tree,
        }
    }

    pub fn get_empty_tree_id(&self) -> Id {
        self.empty_tree_id.clone()
    }

    pub fn get_root_tree_id(&self) -> Id {
        self.root_tree.clone()
    }

    pub fn write_inode(&self, inode: InodeAttributes) {
        let mut inode_store = self.inode_store.lock().unwrap();
        inode_store.insert(inode.inode, inode);
    }

    pub fn get_inode(&self, inode: Inode) -> Option<InodeAttributes> {
        let mut inode_store = self.inode_store.lock().unwrap();
        inode_store.get(&inode).cloned()
    }
}
