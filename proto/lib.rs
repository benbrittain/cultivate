pub mod jj_interface {
    tonic::include_proto!("jj_interface");
}

pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("grpc_descriptor");
