pub mod backend {
    tonic::include_proto!("backend");
}

pub mod control {
    tonic::include_proto!("control");
}

pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("grpc_descriptor");
