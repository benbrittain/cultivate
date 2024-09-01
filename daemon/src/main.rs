use tonic::transport::Server;
use tracing::info;

mod store;
mod service;

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

    info!("daemon started");

    let jj_svc = service::JujutsuService::new();

    let _store = store::Store::new();

    let reflection_svc = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
        .build()?;

    info!("Serving jj gRPC interface");
    Server::builder()
        .add_service(reflection_svc)
        .add_service(jj_svc)
        .serve(addr)
        .await?;

    Ok(())
}
