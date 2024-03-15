use std::sync::{Arc, Mutex};

use proto::backend::{
    backend_client::BackendClient, Commit, CommitId, File, FileId, GetEmptyTreeIdReq,
    SetActiveSnapshotReply, Tree, TreeId,
};
use tokio::runtime::{Builder, Runtime};

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;
type Result<T, E = StdError> = ::std::result::Result<T, E>;

// The order of the fields in this struct is important. They must be ordered
// such that when `BlockingBackendClient` is dropped the client is dropped
// before the runtime. Not doing this will result in a deadlock when dropped.
// Rust drops struct fields in declaration order.
#[derive(Debug)]
pub struct BlockingBackendClient {
    client: Arc<Mutex<BackendClient<tonic::transport::Channel>>>,
    rt: Runtime,
}

impl BlockingBackendClient {
    pub fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
    where
        D: TryInto<tonic::transport::Endpoint>,
        D::Error: Into<StdError>,
    {
        let rt = Builder::new_multi_thread().enable_all().build().unwrap();
        let client = Arc::new(Mutex::new(rt.block_on(BackendClient::connect(dst))?));

        Ok(Self { client, rt })
    }

    pub fn set_active_snapshot(
        &self,
        request: impl tonic::IntoRequest<TreeId>,
    ) -> Result<tonic::Response<SetActiveSnapshotReply>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt.block_on(client.set_active_snapshot(request))
    }

    pub fn write_commit(
        &self,
        request: impl tonic::IntoRequest<Commit>,
    ) -> Result<tonic::Response<CommitId>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt.block_on(client.write_commit(request))
    }

    pub fn read_commit(
        &self,
        request: impl tonic::IntoRequest<CommitId>,
    ) -> Result<tonic::Response<Commit>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt.block_on(client.read_commit(request))
    }

    pub fn write_file(
        &self,
        request: impl tonic::IntoRequest<File>,
    ) -> Result<tonic::Response<FileId>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt.block_on(client.write_file(request))
    }

    pub fn read_file(
        &self,
        request: impl tonic::IntoRequest<FileId>,
    ) -> Result<tonic::Response<File>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt.block_on(client.read_file(request))
    }

    pub fn write_tree(
        &self,
        request: impl tonic::IntoRequest<Tree>,
    ) -> Result<tonic::Response<TreeId>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt.block_on(client.write_tree(request))
    }

    pub fn read_tree(
        &self,
        request: impl tonic::IntoRequest<TreeId>,
    ) -> Result<tonic::Response<Tree>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt.block_on(client.read_tree(request))
    }

    pub fn get_empty_tree_id(&self) -> Result<tonic::Response<TreeId>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt
            .block_on(client.get_empty_tree_id(GetEmptyTreeIdReq::default()))
    }
}
