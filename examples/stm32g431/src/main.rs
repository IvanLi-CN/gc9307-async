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
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::prelude::RgbColor;

use gc9307_async::{Config as DisplayConfig, GC9307C, Orientation};
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

// Display buffer - needs to be static for the lifetime requirement
static mut DISPLAY_BUFFER: [u8; gc9307_async::BUF_SIZE] = [0; gc9307_async::BUF_SIZE];

// SPI bus mutex for sharing between tasks
static DISPLAY_SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, Spi<'static, embassy_stm32::mode::Async>>> = StaticCell::new();

// Embassy timer implementation for gc9307-async
struct EmbassyTimer;

impl gc9307_async::Timer for EmbassyTimer {
    async fn after_millis(milliseconds: u64) {
        embassy_time::Timer::after_millis(milliseconds).await;
    }
}

// Helper function to fill the current address window with a color
// Since we can't access private methods, we'll use write_area as a workaround
async fn fill_current_window<SPI, DC, RST>(
    display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>,
    color: Rgb565,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
) -> Result<(), gc9307_async::Error<SPI::Error>>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    // The write_area method has a limitation: it can only handle MAX_DATA_LEN pixels
    // MAX_DATA_LEN = BUF_SIZE / 2 = 24 * 48 * 2 / 2 = 1152 pixels
    // Each byte in the bitmap represents 8 pixels, so max bitmap size is 1152/8 = 144 bytes

    const MAX_PIXELS: usize = 1152; // gc9307_async::BUF_SIZE / 2
    const MAX_BITMAP_BYTES: usize = MAX_PIXELS / 8; // 144 bytes

    let total_pixels = (width as usize) * (height as usize);

    if total_pixels > MAX_PIXELS {
        // For large areas, fall back to fill_color (which fills entire screen)
        return display.fill_color(color).await;
    }

    // Calculate bitmap data size for 1-bit bitmap (1 bit per pixel)
    let bytes_needed = (total_pixels + 7) / 8; // Round up to nearest byte

    // Create bitmap data filled with 0xFF (all pixels are foreground color)
    let bitmap_data = [0xFF_u8; MAX_BITMAP_BYTES];
    let actual_bytes = bytes_needed.min(MAX_BITMAP_BYTES);

    // Use write_area to draw the filled rectangle
    display.write_area(x, y, width, &bitmap_data[..actual_bytes], color, Rgb565::BLACK).await
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("GC9307 Display Test Patterns Example Starting...");

    let p = embassy_stm32::init(Default::default());
    
    // Configure SPI1 for display communication
    // SCK: PA5, MOSI: PA7
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

    // Configure control pins (from reference project)
    let dc = Output::new(p.PC14, Level::Low, Speed::High);   // Data/Command
    let rst = Output::new(p.PC15, Level::Low, Speed::High);  // Reset
    let cs = Output::new(p.PA15, Level::High, Speed::High);  // Chip Select

    // Create SPI device with chip select
    let spi = SpiDevice::new(spi_bus, cs);
    
    // Configure display
    let display_config = DisplayConfig {
        rgb: false,
        inverted: false,
        orientation: Orientation::Landscape,
        height: 172,
        width: 320,
        dx: 0,
        dy: 34,
    };
    
    // Initialize display
    let buffer = unsafe { &mut *core::ptr::addr_of_mut!(DISPLAY_BUFFER) };
    let mut display = GC9307C::<_, _, _, EmbassyTimer>::new(
        display_config,
        spi,
        dc,
        rst,
        buffer,
    );
    
    info!("Initializing display...");
    if let Err(_e) = display.init().await {
        error!("Display initialization failed");
        return;
    }
    info!("Display initialized successfully!");
    
    // Main loop - cycle through test patterns
    let mut pattern_index = 0;
    loop {
        match pattern_index {
            0 => {
                info!("Drawing solid colors...");
                draw_solid_colors(&mut display).await;
            }
            1 => {
                info!("Drawing color stripes...");
                draw_color_stripes(&mut display).await;
            }
            2 => {
                info!("Drawing checkerboard...");
                draw_checkerboard(&mut display).await;
            }
            3 => {
                info!("Drawing nested rectangles...");
                draw_nested_rectangles(&mut display).await;
            }
            _ => pattern_index = 0,
        }
        
        pattern_index = (pattern_index + 1) % 4;
        // Wait 3 seconds before next pattern
        embassy_time::Timer::after_secs(3).await;
    }
}

// Draw solid colors: Red, Green, Blue
async fn draw_solid_colors<SPI, DC, RST>(
    display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>
) where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    let colors = [
        Rgb565::RED,
        Rgb565::GREEN, 
        Rgb565::BLUE,
    ];
    
    for color in colors.iter() {
        info!("Filling with color");
        if let Err(_e) = display.fill_color(*color).await {
            error!("Failed to fill color");
        }
        embassy_time::Timer::after_millis(1000).await; // 1 second per color
    }
}

// Draw vertical color stripes using proper address window filling
async fn draw_color_stripes<SPI, DC, RST>(
    display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>
) where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    let colors = [
        Rgb565::RED,
        Rgb565::GREEN,
        Rgb565::BLUE,
        Rgb565::YELLOW,
        Rgb565::CYAN,
        Rgb565::MAGENTA,
        Rgb565::WHITE,
    ];

    // Clear screen first
    let _ = display.fill_color(Rgb565::BLACK).await;
    embassy_time::Timer::after_millis(100).await;

    let stripe_width = 320 / colors.len() as u16; // ~45 pixels per stripe

    info!("Drawing {} vertical stripes, each {} pixels wide", colors.len(), stripe_width);

    // Draw each stripe as a rectangle
    for (i, color) in colors.iter().enumerate() {
        let x_start = i as u16 * stripe_width;
        let width = if i == colors.len() - 1 {
            320 - x_start // Last stripe fills to the edge
        } else {
            stripe_width
        };

        info!("Drawing stripe {} at x={}, width={}", i, x_start, width);

        // Set address window for this stripe
        if let Err(_e) = display.set_address_window(x_start, 0, x_start + width - 1, 171).await {
            error!("Failed to set address window for stripe {}", i);
            continue;
        }

        // Fill the stripe area with the color
        if let Err(_e) = fill_current_window(display, *color, x_start, 0, width, 172).await {
            error!("Failed to fill stripe {}", i);
        }

        // Small delay to make the drawing visible
        embassy_time::Timer::after_millis(300).await;
    }
}

// Draw checkerboard pattern
async fn draw_checkerboard<SPI, DC, RST>(
    display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>
) where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    info!("Drawing checkerboard pattern");

    // Clear screen first
    let _ = display.fill_color(Rgb565::BLACK).await;
    embassy_time::Timer::after_millis(100).await;

    let square_size = 20; // 20x20 pixel squares
    let cols = 320 / square_size; // 16 columns
    let rows = 172 / square_size; // 8 rows (with some remainder)

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

            info!("Drawing white square at ({}, {})", x, y);

            // Set address window for this square
            if let Err(_e) = display.set_address_window(
                x, y,
                x + square_size - 1,
                y + square_size - 1
            ).await {
                error!("Failed to set address window for square at ({}, {})", x, y);
                continue;
            }

            // Fill this square with white
            if let Err(_e) = fill_current_window(display, Rgb565::WHITE, x, y, square_size, square_size).await {
                error!("Failed to fill square at ({}, {})", x, y);
            }
        }

        // Small delay per row to make drawing visible
        embassy_time::Timer::after_millis(100).await;
    }
}

// Draw nested rectangles
async fn draw_nested_rectangles<SPI, DC, RST>(
    display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>
) where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    info!("Drawing nested rectangles pattern");

    // Clear screen first
    let _ = display.fill_color(Rgb565::BLACK).await;
    embassy_time::Timer::after_millis(100).await;

    let colors = [
        Rgb565::RED,
        Rgb565::GREEN,
        Rgb565::BLUE,
        Rgb565::YELLOW,
        Rgb565::CYAN,
        Rgb565::MAGENTA,
    ];

    let margin = 15; // Margin between rectangles

    info!("Drawing {} nested rectangles with {} pixel margins", colors.len(), margin);

    // Draw from outside to inside
    for (i, color) in colors.iter().enumerate() {
        let offset = i as u16 * margin;

        // Calculate rectangle bounds
        let x = offset;
        let y = offset;
        let width = if 320 > offset * 2 { 320 - offset * 2 } else { 0 };
        let height = if 172 > offset * 2 { 172 - offset * 2 } else { 0 };

        // Skip if rectangle is too small
        if width < 6 || height < 6 {
            info!("Skipping rectangle {} - too small", i);
            break;
        }

        info!("Drawing rectangle {}: ({}, {}) size {}x{}", i, x, y, width, height);

        // Draw rectangle border by drawing 4 lines
        let border_width = 3;

        // Top border
        for dy in 0..border_width {
            if let Err(_e) = display.set_address_window(x, y + dy, x + width - 1, y + dy).await {
                error!("Failed to set address window for top border");
                continue;
            }
            let _ = fill_current_window(display, *color, x, y + dy, width, 1).await;
        }

        // Bottom border
        for dy in 0..border_width {
            let bottom_y = y + height - 1 - dy;
            if let Err(_e) = display.set_address_window(x, bottom_y, x + width - 1, bottom_y).await {
                error!("Failed to set address window for bottom border");
                continue;
            }
            let _ = fill_current_window(display, *color, x, bottom_y, width, 1).await;
        }

        // Left border
        for dx in 0..border_width {
            if let Err(_e) = display.set_address_window(x + dx, y, x + dx, y + height - 1).await {
                error!("Failed to set address window for left border");
                continue;
            }
            let _ = fill_current_window(display, *color, x + dx, y, 1, height).await;
        }

        // Right border
        for dx in 0..border_width {
            let right_x = x + width - 1 - dx;
            if let Err(_e) = display.set_address_window(right_x, y, right_x, y + height - 1).await {
                error!("Failed to set address window for right border");
                continue;
            }
            let _ = fill_current_window(display, *color, right_x, y, 1, height).await;
        }

        // Delay to make drawing visible
        embassy_time::Timer::after_millis(400).await;
    }
}
