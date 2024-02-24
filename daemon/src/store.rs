use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use prost::Message;
use proto::backend::{Commit, File, Tree};

pub type Id = Vec<u8>;

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
            let empty_tree_id = blake3::hash(&tree.encode_to_vec()).as_bytes().to_vec();
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
