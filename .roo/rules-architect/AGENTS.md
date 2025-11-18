# Project Architecture Rules (Non-Obvious Only)

- **Dual-mode architecture** - Device operates in HighPerformanceMode (USB) or RecorderMode (standalone)
- **Virtual filesystem** - FAT32 filesystem implemented via `emfat` library in RAM
- **Data compression pipeline** - Frequency data → change detection → heatshrink compression → page-based Flash storage
- **QSPI Flash management** - Custom `qspi-stm32lx3` library for MT25QU01GBBB8E12 (128MB)
- **Protobuf communication** - All device communication uses protobuf messages
- **FreeRTOS threading** - Multiple threads for sensors, USB, VFS, and data processing
- **Memory partitioning** - Special SETTINGS section in last 2KB of FLASH for configuration