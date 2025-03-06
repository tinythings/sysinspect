fn main() {
    println!("cargo:rerun-if-changed=proto/ipc.proto");
    tonic_build::compile_protos("proto/ipc.proto").expect("Failed to compile gRPC definitions");
}
