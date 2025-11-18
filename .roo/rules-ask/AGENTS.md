# Project Documentation Rules (Non-Obvious Only)

- **Hardware-specific** - Code is tightly coupled to STM32L433 microcontroller
- **External dependencies** - Requires 24MHz crystal and BOOT0 pin grounded
- **Data decoder** - Separate decoder tool needed for reading compressed Flash data
- **Virtual filesystem structure** - USB presents FAT32 with specific file layout:
  - `readme.txt` - Device description
  - `driver.inf` - Windows 7 driver
  - `proto.prt` - Protobuf protocol definition
  - `config.var` - JSON device settings
  - `storage.var` - JSON Flash status info
  - `data.var` - Compressed measurement data
  - `data_raw.hs` - Raw Flash content
- **Testing approach** - Python integration tests with physical device, no Rust unit tests
- **Custom libraries** - Uses modified forks of USB and compression libraries