use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use prost::Message;
use proto::backend::{backend_server::Backend, *};
use tonic::{Request, Response, Status};

type Id = Vec<u8>;

#[derive(Debug)]
pub struct BackendService {
    commits: Arc<Mutex<HashMap<Id, Commit>>>,
    trees: Arc<Mutex<HashMap<Id, Tree>>>,
    files: Arc<Mutex<HashMap<Id, File>>>,
    empty_tree_id: Vec<u8>,
}

impl BackendService {
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
        BackendService {
            empty_tree_id,
            commits,
            trees,
            files,
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

    async fn write_file(&self, request: Request<File>) -> Result<Response<FileId>, Status> {
        let file = request.into_inner();
        let file_id = blake3::hash(&file.encode_to_vec()).as_bytes().to_vec();
        dbg!(&file_id);
        let mut files = self.files.lock().unwrap();
        files.insert(file_id.clone(), file);
        Ok(Response::new(FileId { file_id }))
    }

    async fn read_file(&self, request: Request<FileId>) -> Result<Response<File>, Status> {
        let file_id = request.into_inner();
        println!("{:x?}", &file_id);
        let files = self.files.lock().unwrap();
        let file = files.get(&file_id.file_id).unwrap();
        Ok(Response::new(file.clone()))
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

        if commit.parents.is_empty() {
            return Err(Status::internal("Cannot write a commit with no parents"));
        }
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

#[cfg(test)]
mod tests {
    const COMMIT_ID_LENGTH: usize = 32;
    const CHANGE_ID_LENGTH: usize = 16;

    use assert_matches::assert_matches;

    use super::*;

    #[tokio::test]
    async fn write_commit_parents() {
        let backend = BackendService::new();
        let mut commit = Commit::default();

        // No parents
        commit.parents = vec![];
        assert_matches!(
            backend.write_commit(Request::new(commit.clone())).await,
            Err(status) if status.message().contains("no parents")
        );

        // Only root commit as parent
        commit.parents = vec![vec![0; CHANGE_ID_LENGTH]];
        let first_id = backend
            .write_commit(Request::new(commit.clone()))
            .await
            .unwrap()
            .into_inner();
        let first_commit = backend
            .read_commit(Request::new(first_id.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(first_commit, commit);

        // Only non-root commit as parent
        commit.parents = vec![first_id.clone().commit_id];
        let second_id = backend
            .write_commit(Request::new(commit.clone()))
            .await
            .unwrap()
            .into_inner();
        let second_commit = backend
            .read_commit(Request::new(second_id.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(second_commit, commit);

        // Merge commit
        commit.parents = vec![first_id.clone().commit_id, second_id.commit_id];
        let merge_id = backend
            .write_commit(Request::new(commit.clone()))
            .await
            .unwrap()
            .into_inner();
        let merge_commit = backend
            .read_commit(Request::new(merge_id.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(merge_commit, commit);

        commit.parents = vec![first_id.commit_id, vec![0; COMMIT_ID_LENGTH]];
        let root_merge_id = backend
            .write_commit(Request::new(commit.clone()))
            .await
            .unwrap()
            .into_inner();
        let root_merge_commit = backend
            .read_commit(Request::new(root_merge_id.clone()))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(root_merge_commit, commit);
    }
}
