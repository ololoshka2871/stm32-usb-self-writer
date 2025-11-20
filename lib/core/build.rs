static PROTOBUF_FILE: &str = "ProtobufDevice_0000E009.proto";
static PROTOBUF_DIR: &str = "src/protobuf/ru.sktbelpa.protobufobjects";

fn main() {
    let mut protofile = std::path::PathBuf::from(PROTOBUF_DIR);
    protofile.push(PROTOBUF_FILE);

    prost_build::compile_protos(&[protofile], &[PROTOBUF_DIR]).unwrap();
}
