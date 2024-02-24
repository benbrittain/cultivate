use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
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

#[derive(Clone, Debug)]
pub struct Store {
    pub commits: Arc<Mutex<HashMap<Id, Commit>>>,
    pub trees: Arc<Mutex<HashMap<Id, Tree>>>,
    pub files: Arc<Mutex<HashMap<Id, File>>>,
    pub empty_tree_id: Id,
}

impl Store {
    pub fn new() -> Self {
        let commits = Arc::new(Mutex::new(HashMap::new()));
        let files = Arc::new(Mutex::new(HashMap::new()));
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
            empty_tree_id,
        }
    }
}
