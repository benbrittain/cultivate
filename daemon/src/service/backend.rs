use prost::Message;
use std::{
    collections::HashMap,
};
use tonic::{Request, Response, Status};

use proto::{backend::backend_server::Backend, backend::*};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct BackendService {
    commits: Arc<Mutex<HashMap<Vec<u8>, Commit>>>,
}

impl BackendService {
    pub fn new() -> Self {
        BackendService {
            commits: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[tonic::async_trait]
impl Backend for BackendService {
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
    async fn write_tree(
        &self,
        _request: Request<WriteTreeRequest>,
    ) -> Result<Response<WriteTreeReply>, Status> {
        todo!()
    }
    async fn read_tree(
        &self,
        _request: Request<ReadTreeRequest>,
    ) -> Result<Response<ReadTreeReply>, Status> {
        todo!()
    }
    async fn write_commit(&self, request: Request<Commit>) -> Result<Response<CommitId>, Status> {
        let commit = request.into_inner();
        dbg!(&commit);
        let commit_id = blake3::hash(&commit.encode_to_vec()).as_bytes().to_vec();
        let mut commits = self.commits.lock().unwrap();
        commits.insert(commit_id.clone(), commit);
        Ok(Response::new(CommitId { commit_id }))
    }

    async fn read_commit(&self, request: Request<CommitId>) -> Result<Response<Commit>, Status> {
        let commit_id = request.into_inner();
        dbg!(&commit_id);
        let commits = self.commits.lock().unwrap();
        let commit = commits.get(&commit_id.commit_id).unwrap();
        Ok(Response::new(commit.clone()))
    }
}
