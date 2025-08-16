# GC9307 Software Rotation Example

This example demonstrates pure software-based screen rotation for the GC9307 display controller.

## Features

- **Pure Software Rotation**: Implements rotation through coordinate transformation, without modifying GC9307 registers
- **Orientation Test Pattern**: Displays a crosshair with colored borders to visualize rotation
- **Automatic Rotation Cycle**: Cycles through 0°, 90°, 180°, 270° rotations every 2.5 seconds

## Test Pattern Description

The orientation test pattern includes:

1. **Center Crosshair**: White cross mark at screen center (3px line width)
2. **Colored Borders**: 3px wide borders in different colors:
   - Top: Red
   - Right: Green  
   - Bottom: Blue
   - Left: Yellow
3. **Corner Marker**: Asymmetric cyan triangle in the top-left corner for orientation reference

## Expected Behavior

- **0°**: Red top, Green right, Blue bottom, Yellow left, Triangle top-left
- **90°**: Yellow top, Red right, Green bottom, Blue left, Triangle top-right
- **180°**: Blue top, Yellow right, Red bottom, Green left, Triangle bottom-right  
- **270°**: Green top, Blue right, Yellow bottom, Red left, Triangle bottom-left

## Hardware Setup

- STM32G431CB microcontroller
- GC9307 display (172×320 resolution)
- SPI connection: SCK=PB3, MOSI=PB5, DC=PC14, RST=PC15, CS=PA15

## Usage

```bash
cargo run --release
```

Monitor the output via RTT/defmt to see rotation state changes and coordinate transformations.
