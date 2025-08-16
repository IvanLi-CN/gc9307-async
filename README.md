# GC9307 Async Driver

**High-Performance Async Display Driver for GC9307 172×320 RGB LCD** 🚀

A fully refactored GC9307 display driver based on [embedded-hal](https://crates.io/crates/embedded-hal), optimized for reliability, performance, and ease of use.

## ✨ Features

- **🔄 Software Rotation** - 0°/90°/180°/270° rotation with coordinate transformation
- **⚡ High Performance** - 16MHz SPI, 512-pixel batching, 4.5x faster rendering
- **🎯 Complete API** - `fill_screen()`, `fill_rect()`, bounds checking, error handling
- **🔧 Easy Integration** - Simple Timer trait, Embassy-time support, comprehensive examples
- **📱 Flexible Configuration** - RGB/BGR order, display offsets, orientation settings

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
gc9307-async = "0.1.1"

# Optional features
gc9307-async = { version = "0.1.1", features = ["software-rotation", "embassy-time"] }
```

### Feature Flags

- `async` (default) - Async SPI support via embedded-hal-async
- `software-rotation` - Enable 4-direction rotation support
- `embassy-time` - Convenience Timer implementation for Embassy users
- `font-rendering` - Font rendering support (planned)

## 🚀 Quick Start

### Basic Usage

```rust
use gc9307_async::{GC9307C, Config, Orientation};
use embedded_graphics::pixelcolor::Rgb565;

// 1. Implement Timer trait
struct MyTimer;
impl gc9307_async::Timer for MyTimer {
    async fn delay_ms(milliseconds: u64) {
        // Your delay implementation
        your_delay_function(milliseconds).await;
    }
}

// 2. Configure display
let config = Config {
    rgb: false,           // BGR color order (common for GC9307)
    inverted: false,      // Normal display
    orientation: Orientation::Landscape,
    width: 320,           // Physical width
    height: 172,          // Physical height
    dx: 0,                // X offset
    dy: 34,               // Y offset (hardware-specific)
};

// 3. Create and initialize display
let mut display = GC9307C::<_, _, _, MyTimer>::new(
    config,
    spi_device,    // Your SPI device
    dc_pin,        // Data/Command pin
    rst_pin,       // Reset pin
    buffer,        // Working buffer (&mut [u8])
);

display.init().await?;

// 4. Start drawing!
display.fill_screen(Rgb565::BLUE).await?;
display.fill_rect(10, 10, 50, 30, Rgb565::RED).await?;
```

### With Embassy-time

```rust
use gc9307_async::{GC9307C, Config, EmbassyTimer};

// Use built-in Embassy timer
let mut display = GC9307C::<_, _, _, EmbassyTimer>::new(
    Config::default(),
    spi_device,
    dc_pin,
    rst_pin,
    buffer,
);
```

## 🎨 Drawing API

### Basic Drawing

```rust
// Fill entire screen
display.fill_screen(Rgb565::BLACK).await?;

// Draw rectangles
display.fill_rect(x, y, width, height, Rgb565::RED).await?;

// Check bounds automatically
let result = display.fill_rect(300, 150, 50, 50, Rgb565::GREEN).await;
// Returns error if rectangle exceeds screen bounds
```

### Software Rotation (Optional Feature)

```rust
// Enable in Cargo.toml: features = ["software-rotation"]
#[cfg(feature = "software-rotation")]
{
    // Set rotation angle
    display.set_rotation(90).await?;  // 0°, 90°, 180°, 270°

    // Logical dimensions change automatically
    let (width, height) = display.logical_dimensions();
    // 0°/180°: (320, 172), 90°/270°: (172, 320)

    // Draw using logical coordinates
    display.fill_rect(0, 0, width/2, height/2, Rgb565::BLUE).await?;
}
```

## ⚙️ Configuration

### Display Config

```rust
let config = Config {
    rgb: false,           // Color order: false=BGR, true=RGB
    inverted: false,      // Display inversion
    orientation: Orientation::Landscape,  // or Portrait
    width: 320,           // Physical width in current orientation
    height: 172,          // Physical height in current orientation
    dx: 0,                // X coordinate offset
    dy: 34,               // Y coordinate offset (common: 34 for GC9307)
};
```

### Common Display Offsets

Different GC9307 modules may require different offsets:

```rust
// Common configurations
let config_type1 = Config { dx: 0, dy: 34, ..Default::default() };   // Most common
let config_type2 = Config { dx: 34, dy: 0, ..Default::default() };   // Alternative
let config_type3 = Config { dx: 0, dy: 0, ..Default::default() };    // No offset
```

## 📊 Performance

Optimized for high-performance rendering:

- **SPI Frequency**: Up to 16MHz (tested on STM32G431)
- **Batch Transfers**: 512-pixel chunks for efficiency
- **Full Screen Fill**: ~0.92 seconds (320×172 pixels)
- **Memory Usage**: Configurable buffer size (minimum 1024 bytes recommended)

## 📚 Examples

Comprehensive examples in the `examples/` directory:

### STM32G431 Complete Example

- **Path**: `examples/stm32g431/`
- **Features**: All test patterns, software rotation, performance benchmarks
- **Hardware**: STM32G431CB + GC9307 172×320 display

### Software Rotation Reference

- **Path**: `examples/stm32g4-software-rotation/`
- **Features**: Pure software rotation demonstration with visual indicators
- **Documentation**: Complete with SVG diagrams

## 🔧 Hardware Setup

### Typical Wiring (SPI)

| MCU Pin | GC9307 Pin | Function |
|---------|------------|----------|
| SCK     | SCK        | SPI Clock |
| MOSI    | SDA        | SPI Data |
| GPIO    | CS         | Chip Select |
| GPIO    | DC         | Data/Command |
| GPIO    | RST        | Reset |
| 3.3V    | VCC        | Power |
| GND     | GND        | Ground |

### Buffer Requirements

```rust
// Minimum recommended buffer size
let mut buffer = [0u8; 1024];  // 1KB buffer

// For better performance
let mut buffer = [0u8; 2048];  // 2KB buffer
```

## 🐛 Troubleshooting

### Common Issues

| Problem | Cause | Solution |
|---------|-------|----------|
| No display | Power/wiring | Check 3.3V supply and connections |
| Wrong colors | Color order | Toggle `rgb: true/false` in config |
| Offset display | Display offset | Adjust `dx`/`dy` values |
| Garbled display | SPI issues | Reduce SPI frequency, check wiring |
| Init failure | Reset timing | Check RST pin, increase delays |

### Debug Tips

```rust
// Enable detailed logging with defmt
use defmt::info;

info!("Display config: {:?}", config);
match display.init().await {
    Ok(()) => info!("Display initialized successfully!"),
    Err(e) => error!("Display initialization failed: {:?}", e),
}
```

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## 📄 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
