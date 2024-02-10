use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path(out_dir.join("grpc_descriptor.bin"))
        .compile(&["backend.proto", "control.proto"], &["."])?;
    Ok(())
}
