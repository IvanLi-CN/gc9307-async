#![no_std]
#![no_main]

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
const CYAN: u16 = 0x07FF;
const MAGENTA: u16 = 0xF81F;

// SPI bus mutex for sharing between tasks
static DISPLAY_SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, Spi<'static, embassy_stm32::mode::Async>>> = StaticCell::new();

/// GC9307 Display driver with direct SPI control
struct Display<SPI, DC, RST> {
    spi: SPI,
    dc: DC,   // Data/Command pin
    rst: RST, // Reset pin
}

impl<SPI, DC, RST> Display<SPI, DC, RST>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    /// Create new display instance
    fn new(spi: SPI, dc: DC, rst: RST) -> Self {
        Self { spi, dc, rst }
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

    /// Set address window for drawing (with offset correction)
    async fn set_address_window(&mut self, x0: u16, y0: u16, x1: u16, y1: u16) -> Result<(), SPI::Error> {
        // Apply display offset
        let x0_offset = x0 + OFFSET_X;
        let y0_offset = y0 + OFFSET_Y;
        let x1_offset = x1 + OFFSET_X;
        let y1_offset = y1 + OFFSET_Y;

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
        self.set_address_window(0, 0, SCREEN_WIDTH - 1, SCREEN_HEIGHT - 1).await?;

        let color_bytes = [(color >> 8) as u8, (color & 0xFF) as u8];
        let total_pixels = SCREEN_WIDTH as u32 * SCREEN_HEIGHT as u32;

        // Send color data for all pixels
        for _ in 0..total_pixels {
            self.write_data_slice(&color_bytes).await?;
        }

        Ok(())
    }

    /// Fill a rectangular area with a color
    async fn fill_rect(&mut self, x: u16, y: u16, width: u16, height: u16, color: u16) -> Result<(), SPI::Error> {
        self.set_address_window(x, y, x + width - 1, y + height - 1).await?;

        let color_bytes = [(color >> 8) as u8, (color & 0xFF) as u8];
        let total_pixels = width as u32 * height as u32;

        // Send color data for all pixels in the rectangle
        for _ in 0..total_pixels {
            self.write_data_slice(&color_bytes).await?;
        }

        Ok(())
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("GC9307 Direct SPI Test Starting...");

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
    
    // Main test loop
    let mut test_index = 0;
    loop {
        match test_index {
            0 => {
                info!("Test 1: Solid Colors");
                test_solid_colors(&mut display).await;
            }
            1 => {
                info!("Test 2: Color Stripes");
                test_color_stripes(&mut display).await;
            }
            2 => {
                info!("Test 3: Checkerboard");
                test_checkerboard(&mut display).await;
            }
            3 => {
                info!("Test 4: Orientation Test (Cross + Borders)");
                test_orientation(&mut display).await;
            }
            _ => test_index = 0,
        }

        test_index = (test_index + 1) % 4;
        embassy_time::Timer::after_secs(3).await;
    }
}

/// Test 1: Cycle through solid colors
async fn test_solid_colors<SPI, DC, RST>(display: &mut Display<SPI, DC, RST>)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    let colors = [RED, GREEN, BLUE, WHITE, BLACK, YELLOW, CYAN, MAGENTA];
    let color_names = ["Red", "Green", "Blue", "White", "Black", "Yellow", "Cyan", "Magenta"];
    
    for (i, &color) in colors.iter().enumerate() {
        info!("Filling with {}", color_names[i]);
        let _ = display.fill_color(color).await;
        embassy_time::Timer::after_millis(1000).await;
    }
}

/// Test 2: Vertical color stripes
async fn test_color_stripes<SPI, DC, RST>(display: &mut Display<SPI, DC, RST>)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    info!("Drawing vertical color stripes...");

    // Clear screen first
    let _ = display.fill_color(BLACK).await;
    embassy_time::Timer::after_millis(100).await;

    let colors = [RED, GREEN, BLUE, YELLOW, CYAN, MAGENTA, WHITE];
    let stripe_width = SCREEN_WIDTH / colors.len() as u16; // ~24 pixels per stripe

    info!("Drawing {} vertical stripes, each {} pixels wide", colors.len(), stripe_width);

    // Draw each stripe
    for (i, &color) in colors.iter().enumerate() {
        let x_start = i as u16 * stripe_width;
        let width = if i == colors.len() - 1 {
            SCREEN_WIDTH - x_start // Last stripe fills to the edge
        } else {
            stripe_width
        };

        info!("Drawing stripe {} at x={}, width={}", i, x_start, width);

        if let Err(_e) = display.fill_rect(x_start, 0, width, SCREEN_HEIGHT, color).await {
            error!("Failed to fill stripe {}", i);
        }

        // Small delay to make the drawing visible
        embassy_time::Timer::after_millis(300).await;
    }
}

/// Test 3: Checkerboard pattern
async fn test_checkerboard<SPI, DC, RST>(display: &mut Display<SPI, DC, RST>)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
{
    info!("Drawing checkerboard pattern...");

    // Clear screen first
    let _ = display.fill_color(BLACK).await;
    embassy_time::Timer::after_millis(100).await;

    let square_size = 20; // 20x20 pixel squares
    let cols = SCREEN_WIDTH / square_size; // 8 columns (172/20 = 8.6, truncated to 8)
    let rows = SCREEN_HEIGHT / square_size; // 16 rows (320/20 = 16)

    info!("Drawing checkerboard: {} cols x {} rows", cols, rows);

    // Draw white squares (black is already the background)
    for row in 0..rows {
        for col in 0..cols {
            // Only draw white squares in checkerboard pattern
            let is_white = (row + col) % 2 == 0;
            if !is_white {
                continue; // Skip black squares
            }

            let x = col * square_size;
            let y = row * square_size;

            if let Err(_e) = display.fill_rect(x, y, square_size, square_size, WHITE).await {
                error!("Failed to fill square at ({}, {})", x, y);
            }
        }

        // Small delay per row to make drawing visible
        embassy_time::Timer::after_millis(50).await;
    }
}
