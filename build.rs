fn build_protobuf(mut cc: cc::Build) {
    let protobuf_src = nanopb_rs_generator::Generator::new()
            .add_proto_file("src/ProtobufDevice_0000E006.proto")
            .generate();

    cc.file(protobuf_src).include("lib/nanopb.rs/nanopb-dist");
    cc.try_compile("protobuf-proto").unwrap_or_else(|e| panic!("{}", e.to_string()));
}

fn build_freertos(mut b: freertos_cargo_build::Builder) {
    // Path to FreeRTOS kernel or set ENV "FREERTOS_SRC" instead
    b.freertos("./FreeRTOS-Kernel");
    b.freertos_port(String::from("GCC/ARM_CM4F")); // Port dir relativ to 'FreeRTOS-Kernel/portable'

    // Location of `FreeRTOSConfig.h`
    if cfg!(debug_assertions) {
        b.freertos_config("src/configDebug");
    } else {
        b.freertos_config("src/configRelease");
    }

    // выбор не работает
    // b.heap(String::from("heap4.c")); // Set the heap_?.c allocator to use from
    // 'FreeRTOS-Kernel/portable/MemMang' (Default: heap_4.c)

    // другие "С"-файлы
    // b.get_cc().file("More.c");   // Optional additional C-Code to be compiled
    b.compile().unwrap_or_else(|e| panic!("{}", e.to_string()));
}

fn main() {
    let mut b = freertos_cargo_build::Builder::new();

    build_protobuf(b.get_cc().clone());
    build_freertos(b);
}
