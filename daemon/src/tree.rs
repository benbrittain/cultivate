use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub type INode = u64;

#[derive(Debug)]
struct Node {
    hash: blake3::Hash,
}

#[derive(Clone, Debug)]
struct TreeStore {
    nodes: Arc<Mutex<HashMap<INode, Node>>>,
}

impl TreeStore {
    pub fn new() -> Self {
        TreeStore {
            nodes: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create() {
        let ts = TreeStore::new();
    }
}
