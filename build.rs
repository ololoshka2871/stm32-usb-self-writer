//use std::{
//    //env, fs,
//    path::{/*Path,*/ PathBuf},
//};
//
//static PROTOBUF_FILE: &str = "ProtobufDevice_0000E009.proto";
//static PROTOBUF_DIR: &str = "src/protobuf/ru.sktbelpa.protobufobjects";
//
//fn gen_protobuf() {
//    let mut protofile = PathBuf::from(PROTOBUF_DIR);
//    protofile.push(PROTOBUF_FILE);
//
//    prost_build::compile_protos(&[protofile], &[PROTOBUF_DIR]).unwrap();
//}

fn main() {
    // deny debug builds
    if cfg!(debug_assertions) {
        panic!("Debug builds are not allowed, use release builds!");
    }

    // Always generate protobuf and FreeRTOS for both normal and test builds
    //gen_protobuf();
    // build_freertos(freertos_cargo_build::Builder::new());
}
