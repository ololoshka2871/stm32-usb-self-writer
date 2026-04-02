use std::path::PathBuf;

static PROTOBUF_FILE: &str = "ProtobufDevice_0000E006.proto";
static PROTOBUF_DIR: &str = "src/protobuf/ru.sktbelpa.protobufobjects";

fn gen_protobuf() {
    let mut protofile = PathBuf::from(PROTOBUF_DIR);
    protofile.push(PROTOBUF_FILE);

    prost_build::compile_protos(&[protofile], &[PROTOBUF_DIR]).unwrap();
}

fn main() {
    // deny debug builds
    if cfg!(debug_assertions) {
        panic!("Debug builds are not allowed, use release builds!");
    }

    #[cfg(all(feature = "xtal-12mhz", feature = "xtal-24mhz"))]
    panic!("Multiple xtal frequency features enabled, choose only one!");

    #[cfg(not(any(feature = "xtal-12mhz", feature = "xtal-24mhz")))]
    panic!("No xtal frequency feature enabled, choose one!");

    gen_protobuf();
}
