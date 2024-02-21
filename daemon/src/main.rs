use tonic::transport::Server;

use proto::{backend::backend_server::BackendServer, control::control_server::ControlServer};

mod service;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let addr = "[::1]:10000".parse()?;

    let control = service::control::ControlService {};
    let control_svc = ControlServer::new(control);

    let backend = service::backend::BackendService::new();
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
