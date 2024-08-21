use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use proto::backend::Commit;

use crate::content_hash::{blake3, ContentHash};

pub type Id = [u8; 32];

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
                ContentHash::update(id.as_slice(), state);
                ContentHash::update(executable, state);
            }
            TreeEntry::TreeId(tree_id) => {
                state.update(&[b'1']);
                ContentHash::update(tree_id.as_slice(), state);
            }
            TreeEntry::SymlinkId(symlink_id) => {
                state.update(&[b'2']);
                ContentHash::update(symlink_id.as_slice(), state);
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
                proto_entry.id = id.to_vec();
                proto_entry.executable = *executable;
                proto::backend::tree_value::Value::File(proto_entry)
            }
            _ => todo!(),
        });
        proto
    }
}

content_hash! {
#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub entries: Vec<(String, TreeEntry)>
}}

impl Tree {
    pub fn get_hash(&self) -> Id {
        *blake3(self).as_bytes()
    }

    pub fn as_proto(&self) -> proto::backend::Tree {
        let mut proto = proto::backend::Tree::default();
        for entry in &self.entries {
            let mut proto_entry = proto::backend::tree::Entry::default();
            proto_entry.name = entry.0.clone();
            proto_entry.value = Some(entry.1.as_proto());
            proto.entries.push(proto_entry);
        }
        proto
    }
}

impl From<proto::backend::TreeValue> for TreeEntry {
    fn from(proto: proto::backend::TreeValue) -> Self {
        let value: proto::backend::tree_value::Value = proto.value.unwrap();
        use proto::backend::tree_value::Value::*;
        match value {
            TreeId(id) => TreeEntry::TreeId(id.try_into().unwrap()),
            SymlinkId(id) => TreeEntry::SymlinkId(id.try_into().unwrap()),
            ConflictId(id) => TreeEntry::ConflictId(id.try_into().unwrap()),
            File(file) => TreeEntry::File {
                id: file.id.try_into().unwrap(),
                executable: file.executable,
            },
        }
    }
}

content_hash! {
#[derive(Clone, Debug, Default)]
pub struct Symlink {
    // TODO maybe represent as PathBuf
    pub target: String,
}
}

impl Symlink {
    pub fn get_hash(&self) -> Id {
        *blake3(self).as_bytes()
    }

    pub fn as_proto(&self) -> proto::backend::Symlink {
        let mut proto = proto::backend::Symlink::default();
        proto.target = self.target.clone();
        proto
    }
}

content_hash! {
#[derive(Clone, Debug, Default)]
pub struct File {
    pub content: Vec<u8>,
}
}

impl File {
    pub fn get_hash(&self) -> Id {
        *blake3(self).as_bytes()
    }

    pub fn as_proto(&self) -> proto::backend::File {
        let mut proto = proto::backend::File::default();
        proto.data = self.content.clone();
        proto
    }
}

impl From<proto::backend::Symlink> for Symlink {
    fn from(proto: proto::backend::Symlink) -> Self {
        let mut symlink = Symlink::default();
        symlink.target = proto.target;
        symlink
    }
}

impl From<proto::backend::File> for File {
    fn from(proto: proto::backend::File) -> Self {
        let mut file = File::default();
        file.content = proto.data;
        file
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

/// Stores mount-agnostic information like Trees or Commits. Unaware of filesystem information.
#[derive(Clone, Debug)]
pub struct Store {
    // Commits
    pub commits: Arc<Mutex<HashMap<Id, Commit>>>,

    /// File contents
    pub files: Arc<Mutex<HashMap<Id, File>>>,

    /// Symlinks
    pub symlinks: Arc<Mutex<HashMap<Id, Symlink>>>,

    /// Empty sha identity
    pub empty_tree_id: Id,

    trees: Arc<Mutex<HashMap<Id, Tree>>>,
}

impl Store {
    pub fn new() -> Self {
        let commits = Arc::new(Mutex::new(HashMap::new()));
        let files = Arc::new(Mutex::new(HashMap::new()));
        let symlinks = Arc::new(Mutex::new(HashMap::new()));

        let (empty_tree_id, trees) = {
            let mut trees = HashMap::new();
            let tree = Tree::default();
            let empty_tree_id: Id = tree.get_hash();
            trees.insert(empty_tree_id.clone(), tree);
            (empty_tree_id, Arc::new(Mutex::new(trees)))
        };

        Store {
            commits,
            trees,
            files,
            symlinks,
            empty_tree_id,
        }
    }

    pub fn get_empty_tree_id(&self) -> Id {
        self.empty_tree_id.clone()
    }

    pub fn get_tree(&self, id: Id) -> Option<Tree> {
        let tree_store = self.trees.lock().unwrap();
        tree_store.get(&id).cloned()
    }

    #[tracing::instrument]
    pub async fn write_tree(&self, tree: Tree) -> Id {
        let mut tree_store = self.trees.lock().unwrap();
        let hash = tree.get_hash();
        tree_store.insert(hash, tree);
        hash
    }

    pub fn get_file(&self, id: Id) -> Option<File> {
        let file_store = self.files.lock().unwrap();
        file_store.get(&id).cloned()
    }

    #[tracing::instrument]
    pub async fn write_file(&self, file: File) -> Id {
        let mut file_store = self.files.lock().unwrap();
        let hash = file.get_hash();
        file_store.insert(hash, file);
        hash
    }

    pub fn get_symlink(&self, id: Id) -> Option<Symlink> {
        let symlink_store = self.symlinks.lock().unwrap();
        symlink_store.get(&id).cloned()
    }

    #[tracing::instrument]
    pub async fn write_symlink(&self, symlink: Symlink) -> Id {
        let mut symlink_store = self.symlinks.lock().unwrap();
        let hash = symlink.get_hash();
        symlink_store.insert(hash, symlink);
        hash
    }
}
