use prost::Message;
use proto::jj_interface::*;
use tonic::{Request, Response, Status};
use tracing::info;

#[derive(Debug)]
pub struct JujutsuService {}

impl JujutsuService {
    pub fn new() -> jujutsu_interface_server::JujutsuInterfaceServer<Self> {
        jujutsu_interface_server::JujutsuInterfaceServer::new(JujutsuService {})
    }
}

#[tonic::async_trait]
impl jujutsu_interface_server::JujutsuInterface for JujutsuService {
    #[tracing::instrument(skip(self))]
    async fn initialize(
        &self,
        request: Request<InitializeReq>,
    ) -> Result<Response<InitializeReply>, Status> {
        let req = request.into_inner();
        info!("Initializing a new repo at {}", req.path);
        todo!();
    }

    #[tracing::instrument(skip(self))]
    async fn get_tree_state(
        &self,
        request: Request<GetTreeStateReq>,
    ) -> Result<Response<GetTreeStateReply>, Status> {
        info!("Getting tree state");
        let _req = request.into_inner();
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn get_checkout_state(
        &self,
        request: Request<GetCheckoutStateReq>,
    ) -> Result<Response<CheckoutState>, Status> {
        info!("Getting checkout state");
        let _req = request.into_inner();
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn set_checkout_state(
        &self,
        request: Request<SetCheckoutStateReq>,
    ) -> Result<Response<SetCheckoutStateReply>, Status> {
        let _req = request.into_inner();
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn snapshot(
        &self,
        request: Request<SnapshotReq>,
    ) -> Result<Response<SnapshotReply>, Status> {
        let _req = request.into_inner();
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn get_empty_tree_id(
        &self,
        _request: Request<GetEmptyTreeIdReq>,
    ) -> Result<Response<TreeId>, Status> {
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn concurrency(
        &self,
        _request: Request<ConcurrencyRequest>,
    ) -> Result<Response<ConcurrencyReply>, Status> {
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn write_file(&self, _request: Request<File>) -> Result<Response<FileId>, Status> {
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn read_file(&self, _request: Request<FileId>) -> Result<Response<File>, Status> {
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn write_symlink(
        &self,
        request: Request<Symlink>,
    ) -> Result<Response<SymlinkId>, Status> {
        let symlink = request.into_inner();
        let _symlink_id = *blake3::hash(&symlink.encode_to_vec()).as_bytes();
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn read_symlink(&self, request: Request<SymlinkId>) -> Result<Response<Symlink>, Status> {
        let _symlink_id = request.into_inner();
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn write_tree(&self, _request: Request<Tree>) -> Result<Response<TreeId>, Status> {
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn read_tree(&self, _request: Request<TreeId>) -> Result<Response<Tree>, Status> {
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn write_commit(&self, request: Request<Commit>) -> Result<Response<CommitId>, Status> {
        let _commit = request.into_inner();
        todo!()
    }

    #[tracing::instrument(skip(self))]
    async fn read_commit(&self, _request: Request<CommitId>) -> Result<Response<Commit>, Status> {
        todo!()
    }
}
