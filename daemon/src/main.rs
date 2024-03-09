use std::path::Path;

use proto::{backend::backend_server::BackendServer, control::control_server::ControlServer};
use tonic::transport::Server;
use tracing::info;

mod fs;
mod service;
mod tree;
#[macro_use]
mod content_hash;
mod mount_store;
mod store;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let addr = "[::1]:10000".parse()?;

    // fuser uses logs, enable for that
    tracing_log::LogTracer::init()?;

    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false)
        .finish();

    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;

    let mut store = store::Store::new();

    info!("Starting mount manager");
    let mut mount_manager = fs::MountManager::new(store.clone());
    mount_manager.mount("/tmp/cultivate")?;

    let control = service::control::ControlService {};
    let control_svc = ControlServer::new(control);

    let backend = service::backend::BackendService::new(store);
    let backend_svc = BackendServer::new(backend);

    let reflection_svc = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()?;

    info!("Serving jj gRPC interface");
    Server::builder()
        .add_service(reflection_svc)
        .add_service(control_svc)
        .add_service(backend_svc)
        .serve(addr)
        .await?;

    Ok(())
}
