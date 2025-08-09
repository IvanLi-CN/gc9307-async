# GC9307 Display Test Patterns Example for STM32G431

This example demonstrates the GC9307 async display driver with various test patterns on STM32G431CBU6.

## Features

The example cycles through four different test patterns every 3 seconds:

1. **Solid Colors** - Red, Green, Blue (1 second each)
2. **Color Stripes** - Vertical stripes in 7 different colors
3. **Checkerboard** - Black and white checkerboard pattern
4. **Nested Rectangles** - Concentric colored rectangles

## Hardware Requirements

- STM32G431CBU6 microcontroller
- GC9307 display (320x172 pixels)
- SWD debugger (e.g., ST-Link)

## Pin Configuration

| Function | STM32G431 Pin | Description |
|----------|---------------|-------------|
| SPI1_SCK | PB3 | SPI Clock |
| SPI1_MOSI | PB5 | SPI Data Out |
| DC | PC14 | Data/Command Control |
| RST | PC15 | Reset |
| CS | PA15 | Chip Select |

## Building and Running

### Prerequisites

1. Install Rust and Cargo
2. Install probe-rs for flashing:
   ```bash
   cargo install probe-rs --features cli
   ```

### Build

```bash
cd examples/stm32g431
cargo build --release
```

### Flash and Run

```bash
cargo run --release
```

Or using probe-rs directly:
```bash
probe-rs run --chip STM32G431CBUx target/thumbv7em-none-eabihf/release/gc9307-stm32g431-example
```

## Debug Output

The example outputs debug information via defmt over RTT. To view the logs:

```bash
# In one terminal, run the program
cargo run --release

# The debug output will show:
# - Initialization status
# - Current pattern being drawn
# - Any errors that occur
```

## Code Structure

- `main.rs` - Main application with Embassy async runtime
- `EmbassyTimer` - Timer implementation for the GC9307 driver
- Pattern drawing functions:
  - `draw_solid_colors()` - Solid color fills
  - `draw_color_stripes()` - Vertical color stripes
  - `draw_checkerboard()` - Checkerboard pattern
  - `draw_nested_rectangles()` - Concentric rectangles

## Display Configuration

- **Resolution**: 320x172 pixels
- **Color Format**: RGB565 (16-bit)
- **Orientation**: Landscape
- **SPI Frequency**: 10 MHz
- **Buffer Size**: 2304 bytes (24x48x2)

## Customization

You can modify the patterns by editing the drawing functions or add new patterns by:

1. Creating a new drawing function following the existing pattern
2. Adding it to the main loop in the `match pattern_index` block
3. Updating the pattern count in the modulo operation

## Troubleshooting

- **Display not working**: Check pin connections and SPI configuration
- **Compilation errors**: Ensure all dependencies are correctly specified
- **Flash errors**: Verify the correct chip is specified in `.cargo/config.toml`
- **No debug output**: Make sure RTT is properly configured and probe-rs supports your debugger

## License

This example is licensed under MIT OR Apache-2.0, same as the parent project.
