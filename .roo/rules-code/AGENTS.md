# Project Coding Rules (Non-Obvious Only)

- **FreeRTOS allocator required** - Must use `freertos_rust::FreeRtosAllocator` as global allocator
- **Dual-mode trait pattern** - All work modes implement `WorkMode<T>` trait with specific initialization sequence
- **Unsafe peripheral access** - Uses `unsafe { cortex_m::Peripherals::take().unwrap_unchecked() }` pattern
- **Feature-gated debug code** - Debug assertions control `master_value_stat` module inclusion
- **Custom CRC configuration** - CRC module uses byte reversal for zlib compatibility (result must be inverted)
- **No-std environment** - Project uses `#![no_std]` with custom allocator and panic handler
- **Protobuf generation** - Build script generates protobuf code from `ProtobufDevice_0000E006.proto`