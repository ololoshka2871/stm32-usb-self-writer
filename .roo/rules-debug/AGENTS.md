# Project Debug Rules (Non-Obvious Only)

- **Defmt logging** - Uses `defmt` framework for logging, not standard println
- **Debug builds blocked** - Cannot build debug version due to memory constraints
- **OpenOCD configuration** - Debugging requires `openocd -f opensda.cfg` setup
- **USB connection detection** - Device mode determined by USB presence at startup
- **Flash sleep mode** - Optional `flash-sleep` feature for power optimization
- **LED debugging** - Optional `led-blink` features for visual status indication