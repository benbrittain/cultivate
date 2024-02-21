use prost::Message;
use std::collections::HashMap;
use tonic::{Request, Response, Status};

use proto::{backend::backend_server::Backend, backend::*};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct BackendService {
    commits: Arc<Mutex<HashMap<Vec<u8>, Commit>>>,
    trees: Arc<Mutex<HashMap<Vec<u8>, Tree>>>,
    empty_tree_id: Vec<u8>,
}

impl BackendService {
    pub fn new() -> Self {
        let commits = Arc::new(Mutex::new(HashMap::new()));
        let (empty_tree_id, trees) = {
            let mut trees = HashMap::new();
            let tree = Tree::default();
            let empty_tree_id = blake3::hash(&tree.encode_to_vec()).as_bytes().to_vec();
            trees.insert(empty_tree_id.clone(), tree);
            (empty_tree_id, Arc::new(Mutex::new(trees)))
        };
        BackendService {
            empty_tree_id,
            commits,
            trees,
        }
    }
}

#[tonic::async_trait]
impl Backend for BackendService {
    async fn get_empty_tree_id(
        &self,
        _request: Request<GetEmptyTreeIdReq>,
    ) -> Result<Response<TreeId>, Status> {
        let tree_id = self.empty_tree_id.clone();
        Ok(Response::new(TreeId { tree_id }))
    }
    async fn concurrency(
        &self,
        _request: Request<ConcurrencyRequest>,
    ) -> Result<Response<ConcurrencyReply>, Status> {
        todo!()
    }
    async fn write_file(
        &self,
        _request: Request<WriteFileRequest>,
    ) -> Result<Response<WriteFileReply>, Status> {
        todo!()
    }
    async fn read_file(
        &self,
        _request: Request<ReadFileRequest>,
    ) -> Result<Response<ReadFileReply>, Status> {
        todo!()
    }
    async fn write_symlink(
        &self,
        _request: Request<WriteSymlinkRequest>,
    ) -> Result<Response<WriteSymlinkReply>, Status> {
        todo!()
    }
    async fn read_symlink(
        &self,
        _request: Request<ReadSymlinkRequest>,
    ) -> Result<Response<ReadSymlinkReply>, Status> {
        todo!()
    }

    async fn write_tree(&self, request: Request<Tree>) -> Result<Response<TreeId>, Status> {
        let tree = request.into_inner();
        let tree_id = blake3::hash(&tree.encode_to_vec()).as_bytes().to_vec();
        dbg!(&tree_id);
        let mut trees = self.trees.lock().unwrap();
        trees.insert(tree_id.clone(), tree);
        Ok(Response::new(TreeId { tree_id }))
    }

    async fn read_tree(&self, request: Request<TreeId>) -> Result<Response<Tree>, Status> {
        let tree_id = request.into_inner();
        println!("{:x?}", &tree_id);
        let trees = self.trees.lock().unwrap();
        let tree = trees.get(&tree_id.tree_id).unwrap();
        Ok(Response::new(tree.clone()))
    }

    async fn write_commit(&self, request: Request<Commit>) -> Result<Response<CommitId>, Status> {
        let commit = request.into_inner();
        let commit_id = blake3::hash(&commit.encode_to_vec()).as_bytes().to_vec();
        let mut commits = self.commits.lock().unwrap();
        commits.insert(commit_id.clone(), commit);
        Ok(Response::new(CommitId { commit_id }))
    }

    async fn read_commit(&self, request: Request<CommitId>) -> Result<Response<Commit>, Status> {
        let commit_id = request.into_inner();
        let commits = self.commits.lock().unwrap();
        let commit = commits.get(&commit_id.commit_id).unwrap();
        Ok(Response::new(commit.clone()))
    }
}
