use tonic::{Request, Response, Status};

use proto::control::{control_server::Control, InitReply, InitRequest};

#[derive(Debug)]
pub struct ControlService;

#[tonic::async_trait]
impl Control for ControlService {
    async fn init(&self, _request: Request<InitRequest>) -> Result<Response<InitReply>, Status> {
        unimplemented!()
    }
}
