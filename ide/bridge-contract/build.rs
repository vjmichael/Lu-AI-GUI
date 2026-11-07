fn main() {
    let protoc = protoc_bin_vendored::protoc_bin_path()
        .expect("vendored protoc required");
    std::env::set_var("PROTOC", protoc);
    prost_build::Config::new()
        .compile_protos(&["../proto/lu/ide/v1/bridge.proto"], &["../proto"])
        .expect("compile shared Lu IDE bridge");
    println!("cargo:rerun-if-changed=../proto/lu/ide/v1/bridge.proto");
}
