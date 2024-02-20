use tonic::{Request, Response, Status};

use proto::{backend::backend_server::Backend, backend::*};

#[derive(Debug)]
pub struct BackendService;

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
    async fn write_commit(
        &self,
        request: Request<Commit>,
    ) -> Result<Response<WriteCommitReply>, Status> {
        dbg!(request);
        let reply = WriteCommitReply {};
        Ok(Response::new(reply))
    }
    async fn read_commit(
        &self,
        _request: Request<ReadCommitRequest>,
    ) -> Result<Response<ReadCommitReply>, Status> {
        todo!()
    }
}
