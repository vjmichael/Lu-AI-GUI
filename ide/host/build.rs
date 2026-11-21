fn main() {
    let protoc =
        protoc_bin_vendored::protoc_bin_path().expect("vendored protoc is required for IDE IPC");
    std::env::set_var("PROTOC", protoc);
    prost_build::Config::new()
        .compile_protos(
            &[
                "../proto/lu/ide/v1/workspace.proto",
                "../proto/lu/ide/v1/native_broker.proto",
                "../proto/lu/workspace/v0/readonly.proto",
            ],
            &["../proto"],
        )
        .expect("compile IDE workspace protobuf schema");
    println!("cargo:rerun-if-changed=../proto/lu/ide/v1/workspace.proto");
    println!("cargo:rerun-if-changed=../proto/lu/ide/v1/native_broker.proto");
    println!("cargo:rerun-if-changed=../proto/lu/workspace/v0/readonly.proto");
}
