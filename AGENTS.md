# AGENTS.md

This file provides guidance to agents when working with code in this repository.

## Build Commands
- **Debug builds are blocked** - Only release builds work: `cargo build --release`
- **Build requires submodules** - Run `git submodule update --init --recursive` first
- **Probe-rs for flashing** - Use `cargo run --release` for automatic debugger detection

## Critical Architecture Constraints
- **Dual-mode operation** - Device switches between `HighPerformanceMode` (USB connected) and `RecorderMode` (standalone)
- **FreeRTOS integration** - Uses custom FreeRTOS build with Rust allocator (`freertos_rust::FreeRtosAllocator`)
- **Memory layout critical** - `memory.x` defines special SETTINGS section at end of FLASH (last 2KB)
- **CRC configuration non-standard** - Uses byte reversal for zlib compatibility (requires result inversion)

## Storage & Data Flow
- **QSPI Flash** - Uses MT25QU01GBBB8E12 (128MB) via custom `qspi-stm32lx3` library
- **Data compression** - All data compressed with `heatshrink` before page-based Flash writes
- **Virtual FAT32 filesystem** - Implemented via `emfat` library with read-only files
- **Protobuf communication** - Protocol defined in `ProtobufDevice_0000E006.proto`

## Testing Specifics
- **Python integration tests** - Run with `SERIAL=/dev/tty<port> py.test -q test/pytest_lib_usb_self_writer.py`
**Rust unit & integration tests (defmt-test)**
- Run unit tests from `mod unit_tests` in `src/lib.rs`:
	- `cargo test --release --lib`
- Run integration tests from `tests/integration_test.rs`:
	- `cargo test --release --test integration_test`
- **Protocol buffer testing** - Tests validate protobuf message exchange

## Hardware Dependencies
- **External 24MHz crystal required** - Must match `XTAL_FREQ` configuration
- **BOOT0 pin must be grounded** - Critical for normal operation
- **STM32L433 specific** - Code heavily tied to this MCU variant