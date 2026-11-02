fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("vendored protoc must be available for Lu's Protobuf contract");
    std::env::set_var("PROTOC", protoc);
    prost_build::Config::new()
        .compile_protos(&["proto/lu/v1/core.proto"], &["proto"])
        .expect("compile Lu's protobuf contract");
    println!("cargo:rerun-if-changed=proto/lu/v1/core.proto");
}
