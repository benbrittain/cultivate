use proto::backend::{backend_client::BackendClient, Commit, WriteCommitReply};
use tokio::runtime::{Builder, Runtime};

use std::sync::{Arc, Mutex};

type StdError = Box<dyn std::error::Error + Send + Sync + 'static>;
type Result<T, E = StdError> = ::std::result::Result<T, E>;

//pub type BlockingBackendClientRef = std::sync::Arc<std::sync::Mutex<BlockingBackendClient>>;

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

    pub fn write_commit(
        &self,
        request: impl tonic::IntoRequest<Commit>,
    ) -> Result<tonic::Response<WriteCommitReply>, tonic::Status> {
        let mut client = self.client.lock().unwrap();
        self.rt.block_on(client.write_commit(request))
    }
}
