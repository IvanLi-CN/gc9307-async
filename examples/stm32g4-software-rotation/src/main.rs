#![no_std]
#![no_main]

// Ensure the linker script is included
extern crate cortex_m_rt;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::spi::{Config, Spi};
use embassy_stm32::time::Hertz;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice;
use embassy_time;
use embedded_hal::digital::OutputPin;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

// Screen dimensions for GC9307 172RGB×320 (portrait orientation)
const SCREEN_WIDTH: u16 = 172;   // Physical width (short edge)
const SCREEN_HEIGHT: u16 = 320;  // Physical height (long edge)
// Display offset (applied to coordinates)
const OFFSET_X: u16 = 34;        // Offset on X axis (short edge)
const OFFSET_Y: u16 = 0;         // No offset on Y axis

// RGB565 color constants
const RED: u16 = 0xF800;
const GREEN: u16 = 0x07E0;
const BLUE: u16 = 0x001F;
const WHITE: u16 = 0xFFFF;
const BLACK: u16 = 0x0000;
const YELLOW: u16 = 0xFFE0;

// 12x16 bitmap font for digits 0-9 and degree symbol
// Each character is 12 pixels wide, 16 pixels tall
// MSB is leftmost pixel (bit 11), LSB is rightmost pixel (bit 0)
const FONT_WIDTH: u16 = 12;
const FONT_HEIGHT: u16 = 16;

// High-quality font data: each u16 represents a row, MSB is leftmost pixel
const FONT_DATA: [[u16; 16]; 11] = [
    // '0' - 12x16 with diagonal slash for better readability
    [
        0b001111111100,
        0b011111111110,
        0b110000000011,
        0b110000000111,
        0b110000001111,
        0b110000011011,
        0b110000110011,
        0b110001100011,
        0b110011000011,
        0b110110000011,
        0b111100000011,
        0b111000000011,
        0b110000000011,
        0b011111111110,
        0b001111111100,
        0b000000000000,
    ],
    // '1' - 12x16 with serif base
    [
        0b000011000000,
        0b000111000000,
        0b001111000000,
        0b011011000000,
        0b000011000000,
        0b000011000000,
        0b000011000000,
        0b000011000000,
        0b000011000000,
        0b000011000000,
        0b000011000000,
        0b000011000000,
        0b000011000000,
        0b011111111100,
        0b011111111100,
        0b000000000000,
    ],
    // '2' - 12x16 with curved top
    [
        0b001111111100,
        0b011111111110,
        0b110000000011,
        0b110000000011,
        0b000000000011,
        0b000000000110,
        0b000000001100,
        0b000000011000,
        0b000000110000,
        0b000001100000,
        0b000011000000,
        0b000110000000,
        0b001100000000,
        0b111111111111,
        0b111111111111,
        0b000000000000,
    ],
    // '3' - 12x16 with middle bar
    [
        0b001111111100,
        0b011111111110,
        0b110000000011,
        0b000000000011,
        0b000000000011,
        0b000000000110,
        0b001111111100,
        0b001111111100,
        0b000000000110,
        0b000000000011,
        0b000000000011,
        0b110000000011,
        0b110000000011,
        0b011111111110,
        0b001111111100,
        0b000000000000,
    ],
    // '4' - 12x16 with clean lines
    [
        0b000000110000,
        0b000001110000,
        0b000011110000,
        0b000110110000,
        0b001100110000,
        0b011000110000,
        0b110000110000,
        0b110000110000,
        0b111111111111,
        0b111111111111,
        0b000000110000,
        0b000000110000,
        0b000000110000,
        0b000000110000,
        0b000000110000,
        0b000000000000,
    ],
    // '5' - 12x16 with horizontal lines
    [
        0b111111111111,
        0b111111111111,
        0b110000000000,
        0b110000000000,
        0b110000000000,
        0b110000000000,
        0b111111111100,
        0b111111111110,
        0b000000000011,
        0b000000000011,
        0b000000000011,
        0b110000000011,
        0b110000000011,
        0b011111111110,
        0b001111111100,
        0b000000000000,
    ],
    // '6' - 12x16 with rounded curves
    [
        0b001111111100,
        0b011111111110,
        0b110000000011,
        0b110000000000,
        0b110000000000,
        0b110000000000,
        0b110111111100,
        0b111111111110,
        0b111000000011,
        0b110000000011,
        0b110000000011,
        0b110000000011,
        0b110000000011,
        0b011111111110,
        0b001111111100,
        0b000000000000,
    ],
    // '7' - 12x16 with angled descent
    [
        0b111111111111,
        0b111111111111,
        0b000000000011,
        0b000000000110,
        0b000000001100,
        0b000000011000,
        0b000000110000,
        0b000001100000,
        0b000011000000,
        0b000110000000,
        0b001100000000,
        0b001100000000,
        0b001100000000,
        0b001100000000,
        0b001100000000,
        0b000000000000,
    ],
    // '8' - 12x16 with double loops
    [
        0b001111111100,
        0b011111111110,
        0b110000000011,
        0b110000000011,
        0b110000000011,
        0b011000000110,
        0b001111111100,
        0b001111111100,
        0b011000000110,
        0b110000000011,
        0b110000000011,
        0b110000000011,
        0b110000000011,
        0b011111111110,
        0b001111111100,
        0b000000000000,
    ],
    // '9' - 12x16 with top loop
    [
        0b001111111100,
        0b011111111110,
        0b110000000011,
        0b110000000011,
        0b110000000011,
        0b110000000011,
        0b110000000111,
        0b011111111111,
        0b001111110011,
        0b000000000011,
        0b000000000011,
        0b000000000011,
        0b110000000011,
        0b011111111110,
        0b001111111100,
        0b000000000000,
    ],
    // '°' (degree symbol) - 12x16 positioned at top
    [
        0b001111110000,
        0b011111111000,
        0b110000001100,
        0b110000001100,
        0b110000001100,
        0b110000001100,
        0b011111111000,
        0b001111110000,
        0b000000000000,
        0b000000000000,
        0b000000000000,
        0b000000000000,
        0b000000000000,
        0b000000000000,
        0b000000000000,
        0b000000000000,
    ],
];

// SPI bus mutex for sharing between tasks
static DISPLAY_SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, Spi<'static, embassy_stm32::mode::Async>>> = StaticCell::new();

/// Software rotation angles
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl Rotation {
    /// Get the next rotation in the cycle
    fn next(self) -> Self {
        match self {
            Rotation::Deg0 => Rotation::Deg90,
            Rotation::Deg90 => Rotation::Deg180,
            Rotation::Deg180 => Rotation::Deg270,
            Rotation::Deg270 => Rotation::Deg0,
        }
    }
    
    /// Get rotation angle in degrees for logging
    fn degrees(self) -> u16 {
        match self {
            Rotation::Deg0 => 0,
            Rotation::Deg90 => 90,
            Rotation::Deg180 => 180,
            Rotation::Deg270 => 270,
        }
    }
}

/// Transform logical coordinates to physical coordinates based on rotation
fn transform_coordinates(x: u16, y: u16, rotation: Rotation, logical_width: u16, logical_height: u16) -> (u16, u16) {
    match rotation {
        Rotation::Deg0 => (x, y),
        Rotation::Deg90 => (logical_height - 1 - y, x),
        Rotation::Deg180 => (logical_width - 1 - x, logical_height - 1 - y),
        Rotation::Deg270 => (y, logical_width - 1 - x),
    }
}

/// Transform a rectangle from logical coordinates to physical coordinates
fn transform_rect(x: u16, y: u16, width: u16, height: u16, rotation: Rotation, logical_width: u16, logical_height: u16) -> (u16, u16, u16, u16) {
    let (x1, y1) = transform_coordinates(x, y, rotation, logical_width, logical_height);
    let (x2, y2) = transform_coordinates(x + width - 1, y + height - 1, rotation, logical_width, logical_height);
    
    let min_x = x1.min(x2);
    let max_x = x1.max(x2);
    let min_y = y1.min(y2);
    let max_y = y1.max(y2);
    
    (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
}

/// GC9307 Display driver with software rotation support
struct Display<SPI, DC, RST> {
    spi: SPI,
    dc: DC,   // Data/Command pin
    rst: RST, // Reset pin
    current_rotation: Rotation,
    logical_width: u16,
    logical_height: u16,
}

impl<SPI, DC, RST> Display<SPI, DC, RST>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    /// Create new display instance
    fn new(spi: SPI, dc: DC, rst: RST) -> Self {
        Self { 
            spi, 
            dc, 
            rst,
            current_rotation: Rotation::Deg0,
            logical_width: SCREEN_WIDTH,
            logical_height: SCREEN_HEIGHT,
        }
    }

    /// Set the current rotation
    fn set_rotation(&mut self, rotation: Rotation) {
        info!("Setting rotation to {}°", rotation.degrees());
        self.current_rotation = rotation;
        
        // Update logical dimensions based on rotation
        match rotation {
            Rotation::Deg0 | Rotation::Deg180 => {
                self.logical_width = SCREEN_WIDTH;
                self.logical_height = SCREEN_HEIGHT;
            }
            Rotation::Deg90 | Rotation::Deg270 => {
                self.logical_width = SCREEN_HEIGHT;
                self.logical_height = SCREEN_WIDTH;
            }
        }
        
        info!("Logical dimensions: {}x{}", self.logical_width, self.logical_height);
    }

    /// Write command to display
    async fn write_command(&mut self, cmd: u8) -> Result<(), SPI::Error> {
        let _ = self.dc.set_low(); // Command mode
        self.spi.write(&[cmd]).await
    }

    /// Write single data byte to display
    async fn write_data(&mut self, data: u8) -> Result<(), SPI::Error> {
        let _ = self.dc.set_high(); // Data mode
        self.spi.write(&[data]).await
    }

    /// Write multiple data bytes to display
    async fn write_data_slice(&mut self, data: &[u8]) -> Result<(), SPI::Error> {
        let _ = self.dc.set_high(); // Data mode
        self.spi.write(data).await
    }

    /// Hardware reset sequence
    async fn reset(&mut self) {
        info!("Performing hardware reset...");
        let _ = self.rst.set_high();
        embassy_time::Timer::after_millis(10).await;
        let _ = self.rst.set_low();
        embassy_time::Timer::after_millis(10).await;
        let _ = self.rst.set_high();
        embassy_time::Timer::after_millis(120).await;
    }

    /// Initialize GC9307 display with complete sequence
    async fn init(&mut self) -> Result<(), SPI::Error> {
        info!("Starting GC9307 initialization...");

        // Hardware reset first
        self.reset().await;

        // Initialization sequence from docs/1.47寸IPS初始化GC9307+HSD.txt
        self.write_command(0xfe).await?;
        self.write_command(0xef).await?;

        self.write_command(0x36).await?;
        self.write_data(0x48).await?;

        self.write_command(0x3a).await?;
        self.write_data(0x05).await?; // 16-bit color

        self.write_command(0x85).await?;
        self.write_data(0xc0).await?;
        self.write_command(0x86).await?;
        self.write_data(0x98).await?;
        self.write_command(0x87).await?;
        self.write_data(0x28).await?;
        self.write_command(0x89).await?;
        self.write_data(0x33).await?;
        self.write_command(0x8B).await?;
        self.write_data(0x84).await?;
        self.write_command(0x8D).await?;
        self.write_data(0x3B).await?;
        self.write_command(0x8E).await?;
        self.write_data(0x0f).await?;
        self.write_command(0x8F).await?;
        self.write_data(0x70).await?;

        self.write_command(0xe8).await?;
        self.write_data(0x13).await?;
        self.write_data(0x17).await?;

        self.write_command(0xec).await?;
        self.write_data(0x57).await?;
        self.write_data(0x07).await?;
        self.write_data(0xff).await?;

        self.write_command(0xed).await?;
        self.write_data(0x18).await?;
        self.write_data(0x09).await?;

        self.write_command(0xc9).await?;
        self.write_data(0x10).await?;

        self.write_command(0xff).await?;
        self.write_data(0x61).await?;

        self.write_command(0x99).await?;
        self.write_data(0x3A).await?;
        self.write_command(0x9d).await?;
        self.write_data(0x43).await?;
        self.write_command(0x98).await?;
        self.write_data(0x3e).await?;
        self.write_command(0x9c).await?;
        self.write_data(0x4b).await?;

        // Gamma correction settings
        self.write_command(0xF0).await?;
        self.write_data(0x06).await?;
        self.write_data(0x08).await?;
        self.write_data(0x08).await?;
        self.write_data(0x06).await?;
        self.write_data(0x05).await?;
        self.write_data(0x1d).await?;

        self.write_command(0xF2).await?;
        self.write_data(0x00).await?;
        self.write_data(0x01).await?;
        self.write_data(0x09).await?;
        self.write_data(0x07).await?;
        self.write_data(0x04).await?;
        self.write_data(0x23).await?;

        self.write_command(0xF1).await?;
        self.write_data(0x3b).await?;
        self.write_data(0x68).await?;
        self.write_data(0x66).await?;
        self.write_data(0x36).await?;
        self.write_data(0x35).await?;
        self.write_data(0x2f).await?;

        self.write_command(0xF3).await?;
        self.write_data(0x37).await?;
        self.write_data(0x6a).await?;
        self.write_data(0x66).await?;
        self.write_data(0x37).await?;
        self.write_data(0x35).await?;
        self.write_data(0x35).await?;

        self.write_command(0xFA).await?;
        self.write_data(0x80).await?;
        self.write_data(0x0f).await?;

        self.write_command(0xBE).await?;
        self.write_data(0x11).await?; // source bias

        self.write_command(0xCB).await?;
        self.write_data(0x02).await?;

        self.write_command(0xCD).await?;
        self.write_data(0x22).await?;

        self.write_command(0x9B).await?;
        self.write_data(0xFF).await?;

        self.write_command(0x35).await?;
        self.write_data(0x00).await?;

        self.write_command(0x44).await?;
        self.write_data(0x00).await?;
        self.write_data(0x0a).await?;

        // Sleep out and display on
        self.write_command(0x11).await?; // Sleep out
        embassy_time::Timer::after_millis(200).await; // Wait 200ms

        self.write_command(0x29).await?; // Display on

        self.write_command(0x2c).await?; // Memory write

        info!("GC9307 initialization completed!");
        Ok(())
    }

    /// Set address window for drawing with software rotation support
    async fn set_address_window(&mut self, logical_x0: u16, logical_y0: u16, logical_x1: u16, logical_y1: u16) -> Result<(), SPI::Error> {
        // Transform logical coordinates to physical coordinates
        let (phys_x0, phys_y0) = transform_coordinates(logical_x0, logical_y0, self.current_rotation, self.logical_width, self.logical_height);
        let (phys_x1, phys_y1) = transform_coordinates(logical_x1, logical_y1, self.current_rotation, self.logical_width, self.logical_height);

        // Ensure we have the correct min/max values
        let min_x = phys_x0.min(phys_x1);
        let max_x = phys_x0.max(phys_x1);
        let min_y = phys_y0.min(phys_y1);
        let max_y = phys_y0.max(phys_y1);

        // Apply display offset
        let x0_offset = min_x + OFFSET_X;
        let y0_offset = min_y + OFFSET_Y;
        let x1_offset = max_x + OFFSET_X;
        let y1_offset = max_y + OFFSET_Y;

        debug!("Address window: logical ({},{}) to ({},{}) -> physical ({},{}) to ({},{}) -> offset ({},{}) to ({},{})",
               logical_x0, logical_y0, logical_x1, logical_y1,
               min_x, min_y, max_x, max_y,
               x0_offset, y0_offset, x1_offset, y1_offset);

        // Column address set
        self.write_command(0x2A).await?;
        self.write_data((x0_offset >> 8) as u8).await?;
        self.write_data((x0_offset & 0xFF) as u8).await?;
        self.write_data((x1_offset >> 8) as u8).await?;
        self.write_data((x1_offset & 0xFF) as u8).await?;

        // Page address set
        self.write_command(0x2B).await?;
        self.write_data((y0_offset >> 8) as u8).await?;
        self.write_data((y0_offset & 0xFF) as u8).await?;
        self.write_data((y1_offset >> 8) as u8).await?;
        self.write_data((y1_offset & 0xFF) as u8).await?;

        // Memory write
        self.write_command(0x2C).await?;
        Ok(())
    }

    /// Fill entire screen with a color
    async fn fill_color(&mut self, color: u16) -> Result<(), SPI::Error> {
        info!("Filling screen with color 0x{:04X}", color);
        self.set_address_window(0, 0, self.logical_width - 1, self.logical_height - 1).await?;

        let color_bytes = [(color >> 8) as u8, (color & 0xFF) as u8];
        let total_pixels = self.logical_width as u32 * self.logical_height as u32;

        // Send color data for all pixels
        for _ in 0..total_pixels {
            self.write_data_slice(&color_bytes).await?;
        }

        Ok(())
    }

    /// Fill a rectangular area with a color (using logical coordinates)
    async fn fill_rect(&mut self, logical_x: u16, logical_y: u16, width: u16, height: u16, color: u16) -> Result<(), SPI::Error> {
        debug!("fill_rect: logical ({},{}) size {}x{} color 0x{:04X}", logical_x, logical_y, width, height, color);

        // For software rotation, we need to handle this pixel by pixel for complex rotations
        // For now, let's use a simple approach for rectangular areas
        match self.current_rotation {
            Rotation::Deg0 | Rotation::Deg180 => {
                // Simple case - can use direct rectangle
                self.set_address_window(logical_x, logical_y, logical_x + width - 1, logical_y + height - 1).await?;
                let color_bytes = [(color >> 8) as u8, (color & 0xFF) as u8];
                let total_pixels = width as u32 * height as u32;
                for _ in 0..total_pixels {
                    self.write_data_slice(&color_bytes).await?;
                }
            }
            Rotation::Deg90 | Rotation::Deg270 => {
                // For 90/270 degree rotations, width and height are swapped
                // We need to draw pixel by pixel or use transformed rectangle
                let (phys_x, phys_y, phys_width, phys_height) = transform_rect(
                    logical_x, logical_y, width, height,
                    self.current_rotation, self.logical_width, self.logical_height
                );

                // Use physical coordinates directly
                let phys_x0 = phys_x;
                let phys_y0 = phys_y;
                let phys_x1 = phys_x + phys_width - 1;
                let phys_y1 = phys_y + phys_height - 1;

                // Apply offset directly to physical coordinates
                let x0_offset = phys_x0 + OFFSET_X;
                let y0_offset = phys_y0 + OFFSET_Y;
                let x1_offset = phys_x1 + OFFSET_X;
                let y1_offset = phys_y1 + OFFSET_Y;

                // Column address set
                self.write_command(0x2A).await?;
                self.write_data((x0_offset >> 8) as u8).await?;
                self.write_data((x0_offset & 0xFF) as u8).await?;
                self.write_data((x1_offset >> 8) as u8).await?;
                self.write_data((x1_offset & 0xFF) as u8).await?;

                // Page address set
                self.write_command(0x2B).await?;
                self.write_data((y0_offset >> 8) as u8).await?;
                self.write_data((y0_offset & 0xFF) as u8).await?;
                self.write_data((y1_offset >> 8) as u8).await?;
                self.write_data((y1_offset & 0xFF) as u8).await?;

                // Memory write
                self.write_command(0x2C).await?;

                let color_bytes = [(color >> 8) as u8, (color & 0xFF) as u8];
                let total_pixels = phys_width as u32 * phys_height as u32;
                for _ in 0..total_pixels {
                    self.write_data_slice(&color_bytes).await?;
                }
            }
        }

        Ok(())
    }

    /// Draw center crosshair (white cross mark)
    async fn draw_crosshair(&mut self) -> Result<(), SPI::Error> {
        let center_x = self.logical_width / 2;
        let center_y = self.logical_height / 2;
        let cross_size = 20;
        let line_width = 3;

        info!("Drawing crosshair at center ({}, {})", center_x, center_y);

        // Horizontal line
        self.fill_rect(
            center_x - cross_size,
            center_y - line_width / 2,
            cross_size * 2,
            line_width,
            WHITE
        ).await?;

        // Vertical line
        self.fill_rect(
            center_x - line_width / 2,
            center_y - cross_size,
            line_width,
            cross_size * 2,
            WHITE
        ).await?;

        Ok(())
    }

    /// Draw colored borders around the screen
    async fn draw_colored_borders(&mut self) -> Result<(), SPI::Error> {
        let border_width = 3;

        info!("Drawing colored borders");

        // Top border - RED
        self.fill_rect(0, 0, self.logical_width, border_width, RED).await?;

        // Right border - GREEN
        self.fill_rect(self.logical_width - border_width, 0, border_width, self.logical_height, GREEN).await?;

        // Bottom border - BLUE
        self.fill_rect(0, self.logical_height - border_width, self.logical_width, border_width, BLUE).await?;

        // Left border - YELLOW
        self.fill_rect(0, 0, border_width, self.logical_height, YELLOW).await?;

        Ok(())
    }

    /// Draw a single character at the specified position
    async fn draw_char(&mut self, x: u16, y: u16, char_index: usize, color: u16) -> Result<(), SPI::Error> {
        if char_index >= FONT_DATA.len() {
            return Ok(()); // Invalid character index
        }

        let char_data = &FONT_DATA[char_index];

        for row in 0..FONT_HEIGHT {
            let row_data = char_data[row as usize];
            for col in 0..FONT_WIDTH {
                // Read from MSB (bit 11) to LSB (bit 0) to avoid mirroring
                if (row_data >> (FONT_WIDTH - 1 - col)) & 1 == 1 {
                    // Draw pixel at (x + col, y + row)
                    self.fill_rect(x + col, y + row, 1, 1, color).await?;
                }
            }
        }

        Ok(())
    }

    /// Draw rotation angle text (e.g., "0°", "90°", "180°", "270°")
    async fn draw_rotation_text(&mut self, rotation: Rotation) -> Result<(), SPI::Error> {
        let angle = rotation.degrees();
        info!("Drawing rotation text: {}°", angle);

        let text_x = 10;
        let text_y = 10;
        let char_spacing = FONT_WIDTH + 2; // 2 pixel spacing between characters

        // Clear the text area first (draw black rectangle) - larger area for 12x16 font
        // Need space for up to 4 characters: "270°" = 4 * (12 + 2) - 2 = 54 pixels wide
        self.fill_rect(text_x, text_y, 54, FONT_HEIGHT, BLACK).await?;

        let mut x_offset = 0;

        // Draw the angle digits
        if angle >= 100 {
            // Draw hundreds digit (only for angles >= 100)
            let hundreds = (angle / 100) as usize;
            self.draw_char(text_x + x_offset, text_y, hundreds, WHITE).await?;
            x_offset += char_spacing;
        }

        if angle >= 10 {
            // Draw tens digit (for angles >= 10)
            let tens = ((angle / 10) % 10) as usize;
            self.draw_char(text_x + x_offset, text_y, tens, WHITE).await?;
            x_offset += char_spacing;
        }

        // Draw units digit
        let units = (angle % 10) as usize;
        self.draw_char(text_x + x_offset, text_y, units, WHITE).await?;
        x_offset += char_spacing;

        // Draw degree symbol (index 10 in our font data)
        self.draw_char(text_x + x_offset, text_y, 10, WHITE).await?;

        Ok(())
    }

    /// Draw corner marker with rotation angle text
    async fn draw_corner_marker(&mut self) -> Result<(), SPI::Error> {
        info!("Drawing rotation angle text");

        // Draw the rotation angle text instead of the L-shaped marker
        self.draw_rotation_text(self.current_rotation).await?;

        Ok(())
    }

    /// Draw complete orientation test pattern
    async fn draw_orientation_test(&mut self) -> Result<(), SPI::Error> {
        info!("Drawing orientation test pattern for {}° rotation", self.current_rotation.degrees());

        // Clear screen first
        self.fill_color(BLACK).await?;
        embassy_time::Timer::after_millis(100).await;

        // Draw all elements
        self.draw_colored_borders().await?;
        embassy_time::Timer::after_millis(50).await;

        self.draw_crosshair().await?;
        embassy_time::Timer::after_millis(50).await;

        self.draw_corner_marker().await?;

        info!("Orientation test pattern completed");
        Ok(())
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("GC9307 Software Rotation Example Starting...");

    let p = embassy_stm32::init(Default::default());

    // Configure SPI1 for display communication
    let mut spi_config = Config::default();
    spi_config.frequency = Hertz(10_000_000); // 10MHz

    let spi_bus = Spi::new_txonly(
        p.SPI1,
        p.PB3,  // SCK
        p.PB5,  // MOSI
        p.DMA1_CH3, // TX DMA
        spi_config,
    );

    // Initialize SPI bus mutex
    let spi_bus = Mutex::new(spi_bus);
    let spi_bus = DISPLAY_SPI_BUS.init(spi_bus);

    // Configure control pins
    let dc = Output::new(p.PC14, Level::Low, Speed::High);   // Data/Command
    let rst = Output::new(p.PC15, Level::Low, Speed::High);  // Reset
    let cs = Output::new(p.PA15, Level::High, Speed::High);  // Chip Select

    // Create SPI device with chip select
    let spi = SpiDevice::new(spi_bus, cs);

    // Create display instance
    let mut display = Display::new(spi, dc, rst);

    // Initialize display
    info!("Initializing display...");
    if let Err(_e) = display.init().await {
        error!("Display initialization failed!");
        return;
    }
    info!("Display initialized successfully!");

    // Phase 1: Test basic functionality with 0° rotation
    info!("=== PHASE 1: Testing 0° rotation ===");
    display.set_rotation(Rotation::Deg0);
    if let Err(_e) = display.draw_orientation_test().await {
        error!("Failed to draw orientation test pattern");
        return;
    }

    // Wait 3 seconds to observe the result
    info!("Phase 1 complete. Waiting 3 seconds before starting rotation cycle...");
    embassy_time::Timer::after_secs(3).await;

    // Phase 2: Rotation cycle demonstration
    info!("=== PHASE 2: Starting rotation cycle ===");
    let mut current_rotation = Rotation::Deg0;

    loop {
        info!("--- Switching to {}° rotation ---", current_rotation.degrees());
        display.set_rotation(current_rotation);

        if let Err(_e) = display.draw_orientation_test().await {
            error!("Failed to draw orientation test for {}°", current_rotation.degrees());
        } else {
            info!("Successfully displayed {}° orientation", current_rotation.degrees());
        }

        // Wait 2.5 seconds before next rotation
        embassy_time::Timer::after_millis(2500).await;

        // Move to next rotation
        current_rotation = current_rotation.next();
    }
}
