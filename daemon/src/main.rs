use std::path::Path;

use proto::{backend::backend_server::BackendServer, control::control_server::ControlServer};
use tonic::transport::Server;

mod tree;
mod fs;
mod service;
#[macro_use]
mod content_hash;
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

    let store = store::Store::new();

    let store2 = store.clone();
    let handler = std::thread::spawn(move || {
        let mount_manager = fs::MountManager::new(store2);
        mount_manager.mount("/tmp/cultivate")?;
        Ok::<(), anyhow::Error>(())
    });

    let control = service::control::ControlService {};
    let control_svc = ControlServer::new(control);

    let backend = service::backend::BackendService::new(store);
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

    handler.join().unwrap()?;

    Ok(())
}
