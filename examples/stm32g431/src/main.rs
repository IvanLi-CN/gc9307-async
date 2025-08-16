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

use gc9307_async::{Config as DisplayConfig, GC9307C, Orientation};
#[cfg(feature = "software-rotation")]
use gc9307_async::Rotation;
use static_cell::StaticCell;
use {defmt_rtt as _, panic_probe as _};

// RGB565 color constants (from successful examples)
const RED: Rgb565 = Rgb565::new(31, 0, 0);
const GREEN: Rgb565 = Rgb565::new(0, 63, 0);
const BLUE: Rgb565 = Rgb565::new(0, 0, 31);
const WHITE: Rgb565 = Rgb565::new(31, 63, 31);
const BLACK: Rgb565 = Rgb565::new(0, 0, 0);
const YELLOW: Rgb565 = Rgb565::new(31, 63, 0);
const CYAN: Rgb565 = Rgb565::new(0, 63, 31);
const MAGENTA: Rgb565 = Rgb565::new(31, 0, 31);

// Display buffer - needs to be static for the lifetime requirement
static mut DISPLAY_BUFFER: [u8; gc9307_async::BUF_SIZE] = [0; gc9307_async::BUF_SIZE];

// SPI bus mutex for sharing between tasks
static DISPLAY_SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, Spi<'static, embassy_stm32::mode::Async>>> = StaticCell::new();

// Embassy timer implementation for gc9307-async
struct EmbassyTimer;

impl gc9307_async::Timer for EmbassyTimer {
    async fn delay_ms(milliseconds: u64) {
        embassy_time::Timer::after_millis(milliseconds).await;
    }
}

/// Test 1: RGB Colors only (simplified)
async fn test_rgb_colors<SPI, DC, RST>(display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    let colors = [RED, GREEN, BLUE];
    let color_names = ["Red", "Green", "Blue"];

    for (i, &color) in colors.iter().enumerate() {
        info!("Filling with {}", color_names[i]);
        let _ = display.fill_screen(color).await;
        embassy_time::Timer::after_millis(800).await; // Faster transitions
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    info!("GC9307 Enhanced Display Test Starting...");

    let p = embassy_stm32::init(Default::default());

    // Configure SPI1 for display communication (maximum safe speed)
    let mut spi_config = Config::default();
    spi_config.frequency = Hertz(16_000_000); // 16MHz - maximum for 16MHz system clock

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

    // Configure control pins (same as successful examples)
    let dc = Output::new(p.PC14, Level::Low, Speed::High);   // Data/Command
    let rst = Output::new(p.PC15, Level::Low, Speed::High);  // Reset
    let cs = Output::new(p.PA15, Level::High, Speed::High);  // Chip Select

    // Create SPI device with chip select
    let spi = SpiDevice::new(spi_bus, cs);

    // Configure display with correct dimensions
    let display_config = DisplayConfig {
        rgb: false,
        inverted: false,
        orientation: Orientation::Landscape,
        height: 172,  // Physical height in landscape mode
        width: 320,   // Physical width in landscape mode
        dx: 0,        // No X offset
        dy: 34,       // Y offset as per successful examples
    };

    // Initialize display with new simplified constructor
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

    // Simplified test loop - only essential tests
    let mut test_index = 0;
    loop {
        match test_index {
            0 => {
                info!("Test 1: RGB Colors");
                test_rgb_colors(&mut display).await;
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
                info!("Test 4: Direction Markers");
                test_direction_markers(&mut display).await;
            }
            _ => test_index = 0,
        }

        test_index = (test_index + 1) % 4;
        embassy_time::Timer::after_secs(2).await; // Faster cycling
    }
}

/// Test 2: Vertical color stripes (from direct-spi example)
async fn test_color_stripes<SPI, DC, RST>(display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    info!("Drawing vertical color stripes...");

    // Clear screen first
    let _ = display.fill_screen(BLACK).await;
    embassy_time::Timer::after_millis(100).await;

    let colors = [RED, GREEN, BLUE, YELLOW, CYAN, MAGENTA, WHITE];
    let stripe_width = 320 / colors.len() as u16; // ~45 pixels per stripe

    info!("Drawing {} vertical stripes, each {} pixels wide", colors.len(), stripe_width);

    // Draw each stripe
    for (i, &color) in colors.iter().enumerate() {
        let x_start = i as u16 * stripe_width;
        let width = if i == colors.len() - 1 {
            320 - x_start // Last stripe fills to the edge
        } else {
            stripe_width
        };

        info!("Drawing stripe {} at x={}, width={}", i, x_start, width);

        if let Err(_e) = display.fill_rect(x_start, 0, width, 172, color).await {
            error!("Failed to fill stripe {}", i);
        }

        // Small delay to make the drawing visible
        embassy_time::Timer::after_millis(300).await;
    }
}

/// Test 3: Checkerboard pattern (from direct-spi example)
async fn test_checkerboard<SPI, DC, RST>(display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    info!("Drawing checkerboard pattern...");

    // Clear screen first
    let _ = display.fill_screen(BLACK).await;
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

            if let Err(_e) = display.fill_rect(x, y, square_size, square_size, WHITE).await {
                error!("Failed to fill square at ({}, {})", x, y);
            }
        }

        // Small delay per row to make drawing visible
        embassy_time::Timer::after_millis(50).await;
    }
}

/// Test 4: Four direction rotation positioning test
async fn test_direction_markers<SPI, DC, RST>(display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    #[cfg(feature = "software-rotation")]
    {
        use gc9307_async::Rotation;

        let rotations = [Rotation::Deg0, Rotation::Deg90, Rotation::Deg180, Rotation::Deg270];

        for &rotation in rotations.iter() {
            let angle = rotation.degrees();
            info!("Setting rotation to {}°", angle);
            display.set_rotation(rotation);

            let (logical_width, logical_height) = display.logical_dimensions();
            info!("Logical dimensions: {}x{}", logical_width, logical_height);

            // Clear screen
            let _ = display.fill_screen(BLACK).await;
            embassy_time::Timer::after_millis(50).await;

            // Draw positioning markers with angle text
            draw_rotation_markers(display, logical_width, logical_height, angle).await;

            // Hold for 2 seconds to observe the angle text
            embassy_time::Timer::after_millis(2000).await;
        }

        // Reset to default rotation
        display.set_rotation(Rotation::Deg0);
    }

    #[cfg(not(feature = "software-rotation"))]
    {
        info!("Drawing static direction markers...");

        // Clear screen
        let _ = display.fill_screen(BLACK).await;
        embassy_time::Timer::after_millis(50).await;

        let marker_size = 30;

        // Top-left marker - RED
        let _ = display.fill_rect(0, 0, marker_size, marker_size, RED).await;

        // Top-right marker - GREEN
        let _ = display.fill_rect(320 - marker_size, 0, marker_size, marker_size, GREEN).await;

        // Bottom-left marker - BLUE
        let _ = display.fill_rect(0, 172 - marker_size, marker_size, marker_size, BLUE).await;

        // Bottom-right marker - WHITE
        let _ = display.fill_rect(320 - marker_size, 172 - marker_size, marker_size, marker_size, WHITE).await;

        // Center cross
        let center_x = 320 / 2;
        let center_y = 172 / 2;
        let cross_size = 15;
        let line_width = 2;

        let _ = display.fill_rect(center_x - cross_size, center_y - line_width / 2, cross_size * 2, line_width, YELLOW).await;
        let _ = display.fill_rect(center_x - line_width / 2, center_y - cross_size, line_width, cross_size * 2, YELLOW).await;
    }

    info!("Direction markers completed");
}

#[cfg(feature = "software-rotation")]
/// Draw rotation markers for software rotation test with angle text
async fn draw_rotation_markers<SPI, DC, RST>(
    display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>,
    logical_width: u16,
    logical_height: u16,
    angle: u16
)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    let marker_size = 25;

    // Four corner markers to show rotation
    // Top-left corner - RED
    let _ = display.fill_rect(0, 0, marker_size, marker_size, RED).await;

    // Top-right corner - GREEN
    let _ = display.fill_rect(logical_width - marker_size, 0, marker_size, marker_size, GREEN).await;

    // Bottom-left corner - BLUE
    let _ = display.fill_rect(0, logical_height - marker_size, marker_size, marker_size, BLUE).await;

    // Bottom-right corner - WHITE
    let _ = display.fill_rect(
        logical_width - marker_size,
        logical_height - marker_size,
        marker_size,
        marker_size,
        WHITE
    ).await;

    // Center cross for reference
    let center_x = logical_width / 2;
    let center_y = logical_height / 2;
    let cross_size = 12;
    let line_width = 3;

    // Horizontal line
    let _ = display.fill_rect(
        center_x - cross_size,
        center_y - line_width / 2,
        cross_size * 2,
        line_width,
        YELLOW
    ).await;

    // Vertical line
    let _ = display.fill_rect(
        center_x - line_width / 2,
        center_y - cross_size,
        line_width,
        cross_size * 2,
        YELLOW
    ).await;

    // Draw angle text in center area with high contrast
    #[cfg(feature = "font-rendering")]
    {
        let text_x = center_x - 20; // Center the text approximately
        let text_y = center_y + 20; // Below the cross
        let _ = display.draw_angle_text(text_x, text_y, angle, CYAN).await;
    }

    // Also draw angle in top-left area for better visibility
    #[cfg(feature = "font-rendering")]
    {
        let text_x = 30; // Right of the red marker
        let text_y = 5;  // Top area
        let _ = display.draw_angle_text(text_x, text_y, angle, WHITE).await;
    }
}



#[cfg(feature = "software-rotation")]
/// Test 6: Software rotation demonstration (from software-rotation example)
async fn test_software_rotation<SPI, DC, RST>(display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    info!("Starting software rotation demonstration...");

    let rotations = [Rotation::Deg0, Rotation::Deg90, Rotation::Deg180, Rotation::Deg270];

    for &rotation in rotations.iter() {
        info!("Setting rotation to {}°", rotation.degrees());
        display.set_rotation(rotation);

        let (logical_width, logical_height) = display.logical_dimensions();
        info!("Logical dimensions: {}x{}", logical_width, logical_height);

        // Clear screen
        let _ = display.fill_screen(BLACK).await;
        embassy_time::Timer::after_millis(100).await;

        // Draw orientation indicators
        draw_rotation_indicators(display, rotation).await;

        // Hold for 2 seconds to observe
        embassy_time::Timer::after_millis(2000).await;
    }

    // Reset to default rotation
    display.set_rotation(Rotation::Deg0);
    info!("Software rotation demonstration completed!");
}

#[cfg(feature = "software-rotation")]
/// Draw rotation indicators for software rotation test
async fn draw_rotation_indicators<SPI, DC, RST>(
    display: &mut GC9307C<'_, SPI, DC, RST, EmbassyTimer>,
    rotation: Rotation
)
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
    RST: embedded_hal::digital::OutputPin<Error = core::convert::Infallible>,
{
    let (logical_width, logical_height) = display.logical_dimensions();

    // Draw colored corners to show rotation
    let corner_size = 30;

    // Top-left corner - RED
    let _ = display.fill_rect(0, 0, corner_size, corner_size, RED).await;

    // Top-right corner - GREEN
    let _ = display.fill_rect(logical_width - corner_size, 0, corner_size, corner_size, GREEN).await;

    // Bottom-left corner - BLUE
    let _ = display.fill_rect(0, logical_height - corner_size, corner_size, corner_size, BLUE).await;

    // Bottom-right corner - YELLOW
    let _ = display.fill_rect(
        logical_width - corner_size,
        logical_height - corner_size,
        corner_size,
        corner_size,
        YELLOW
    ).await;

    // Draw center cross
    let center_x = logical_width / 2;
    let center_y = logical_height / 2;
    let cross_size = 20;
    let line_width = 4;

    // Horizontal line
    let _ = display.fill_rect(
        center_x - cross_size,
        center_y - line_width / 2,
        cross_size * 2,
        line_width,
        WHITE
    ).await;

    // Vertical line
    let _ = display.fill_rect(
        center_x - line_width / 2,
        center_y - cross_size,
        line_width,
        cross_size * 2,
        WHITE
    ).await;

    // Draw angle indicator in top-left area
    let angle_text_color = match rotation {
        Rotation::Deg0 => CYAN,
        Rotation::Deg90 => MAGENTA,
        Rotation::Deg180 => YELLOW,
        Rotation::Deg270 => WHITE,
    };

    // Simple angle indicator - draw small rectangles to represent the angle
    let indicator_x = 40;
    let indicator_y = 40;
    let bar_width = 20;
    let bar_height = 4;

    match rotation {
        Rotation::Deg0 => {
            // Horizontal bar
            let _ = display.fill_rect(indicator_x, indicator_y, bar_width, bar_height, angle_text_color).await;
        }
        Rotation::Deg90 => {
            // Vertical bar
            let _ = display.fill_rect(indicator_x, indicator_y, bar_height, bar_width, angle_text_color).await;
        }
        Rotation::Deg180 => {
            // Horizontal bar (same as 0° but different color)
            let _ = display.fill_rect(indicator_x, indicator_y, bar_width, bar_height, angle_text_color).await;
        }
        Rotation::Deg270 => {
            // Vertical bar (same as 90° but different color)
            let _ = display.fill_rect(indicator_x, indicator_y, bar_height, bar_width, angle_text_color).await;
        }
    }
}
