use proto::control::{control_server::Control, InitReply, InitRequest, StatusReply, StatusRequest};
use tonic::{Request, Response, Status};

#[derive(Debug)]
pub struct ControlService;

#[tonic::async_trait]
impl Control for ControlService {
    async fn init(&self, _request: Request<InitRequest>) -> Result<Response<InitReply>, Status> {
        unimplemented!()
    }
    async fn status(
        &self,
        _request: Request<StatusRequest>,
    ) -> Result<Response<StatusReply>, Status> {
        unimplemented!()
    }
}
