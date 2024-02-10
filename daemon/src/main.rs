use tonic::transport::Server;
use tonic::{Request, Response, Status};

use proto::{
    backend_server::{Backend, BackendServer},
    control_server::{Control, ControlServer},
    InitReply, InitRequest, NameReply, NameRequest,
};

#[derive(Debug)]
struct BackendService;

#[tonic::async_trait]
impl Backend for BackendService {
    async fn name(&self, _request: Request<NameRequest>) -> Result<Response<NameReply>, Status> {
        unimplemented!()
    }
}

#[derive(Debug)]
struct ControlService;

#[tonic::async_trait]
impl Control for ControlService {
    async fn init(&self, _request: Request<InitRequest>) -> Result<Response<InitReply>, Status> {
        unimplemented!()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:10000".parse().unwrap();

    let control = ControlService {};
    let control_svc = ControlServer::new(control);

    let backend = BackendService {};
    let backend_svc = BackendServer::new(backend);

    let reflection_svc = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()?;

    Server::builder()
        .add_service(reflection_svc)
        .add_service(control_svc)
        .add_service(backend_svc)
        .serve(addr)
        .await?;

    Ok(())
}
