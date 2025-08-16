#![no_std]

use core::convert::Infallible;

use embedded_graphics_core::pixelcolor::{Rgb565, raw::RawU16};
use embedded_graphics_core::prelude::RawData;
use embedded_hal::digital::OutputPin;
#[cfg(not(feature = "async"))]
use embedded_hal::spi::SpiDevice;
#[cfg(feature = "async")]
use embedded_hal_async::spi::SpiDevice;

// Screen dimensions for GC9307 172RGB×320
pub const SCREEN_WIDTH: u16 = 172; // Physical width (short edge)
pub const SCREEN_HEIGHT: u16 = 320; // Physical height (long edge)
// Display offset (applied to coordinates)
pub const OFFSET_X: u16 = 34; // Offset on X axis (short edge)
pub const OFFSET_Y: u16 = 0; // No offset on Y axis

// Buffer size for chunked operations
pub const BUF_SIZE: usize = 24 * 48 * 2;
const MAX_DATA_LEN: usize = BUF_SIZE / 2;

#[derive(Debug, Clone, Copy)]
pub enum Instruction {
    /// Read Display Identification (04h) - Returns manufacturer and version information
    ReadDisplayId = 0x04,
    /// Read Display Status (09h) - Checks display operating state
    ReadDisplayStatus = 0x09,

    /// Sleep In (10h) - Enter low-power mode
    SleepIn = 0x10,
    /// Sleep Out (11h) - Exit low-power mode
    SleepOut = 0x11,
    /// Partial Display Mode On (12h) - Enable regional refresh
    PartialModeOn = 0x12,
    /// Normal Display Mode On (13h) - Full-screen mode
    NormalDisplayOn = 0x13,

    /// Display Inversion Off (20h) - Disable color inversion
    DisplayInversionOff = 0x20,
    /// Display Inversion On (21h) - Enable color inversion
    DisplayInversionOn = 0x21,

    /// Display Off (28h) - Disable panel output
    DisplayOff = 0x28,
    /// Display On (29h) - Enable panel output
    DisplayOn = 0x29,
    /// Column Address Set (2Ah) - Horizontal addressing bounds
    ColumnAddressSet = 0x2A,
    /// Page Address Set (2Bh) - Vertical addressing bounds
    PageAddressSet = 0x2B,
    /// Memory Write (2Ch) - Write to memory
    MemoryWrite = 0x2C,

    /// Tearing Effect Line On (35h) - Enable VSync output
    TearingEffectEnable = 0x35,
    /// Memory Access Control (36h) - GRAM orientation/order
    MemoryAccessControl = 0x36,
    /// Pixel Format Set (3Ah) - Color depth configuration
    PixelFormatSet = 0x3A,

    /// Tearing Effect Control (44h) - VSync line address
    TearingEffectControl = 0x44,

    /// VCore Voltage Regulation (A7h) - Core voltage adjustment
    VcoreVoltageControl = 0xA7,

    /// RGB Interface Control (B0h) - Signal timing parameters
    RgbInterfaceControl = 0xB0,
    /// Blanking Porch Control (B5h) - Vertical/horizontal timing
    BlankingPorchControl = 0xB5,
    /// Display Function Control (B6h) - Scan direction/number
    DisplayFunctionControl = 0xB6,

    /// Power Control 1 (C1h) - Main voltage regulation
    PowerControl1 = 0xC1,
    /// VREG1A Control (C3h) - Positive charge pump
    Vreg1aControl = 0xC3,
    /// VREG1B Control (C4h) - Negative charge pump
    Vreg1bControl = 0xC4,
    /// VREG2A Control (C9h) - Analog voltage regulator
    Vreg2aControl = 0xC9,

    /// Frame Rate Control (E8h) - Refresh rate configuration
    FrameRateControl = 0xE8,
    /// SPI Interface Control (E9h) - Protocol configuration
    SpiInterfaceControl = 0xE9,

    /// Interface Configuration (F6h) - Bus protocol settings
    InterfaceConfiguration = 0xF6,

    /// Gamma Set 1 (F0h) - Primary gamma correction
    GammaSet1 = 0xF0,
    /// Gamma Set 2 (F1h) - Secondary gamma correction
    GammaSet2 = 0xF1,
    /// Gamma Set 3 (F2h) - Fast transition adjustment
    GammaSet3 = 0xF2,
    /// Gamma Set 4 (F3h) - Slow transition adjustment
    GammaSet4 = 0xF3,

    /// Extended Register Access 2 (EFh) - Advanced command mode
    ExtendedRegAccess2 = 0xEF,
    /// Extended Register Access 1 (FEh) - Basic command mode
    ExtendedRegAccess1 = 0xFE,
}

#[derive(Clone, Copy)]
pub enum Orientation {
    Portrait = 0x40,
    Landscape = 0x20,
    PortraitSwapped = 0x80,
    LandscapeSwapped = 0xE0,
}

#[cfg(feature = "software-rotation")]
/// Software rotation angles
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rotation {
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

#[cfg(feature = "software-rotation")]
impl Rotation {
    /// Get the next rotation in the cycle
    pub fn next(self) -> Self {
        match self {
            Rotation::Deg0 => Rotation::Deg90,
            Rotation::Deg90 => Rotation::Deg180,
            Rotation::Deg180 => Rotation::Deg270,
            Rotation::Deg270 => Rotation::Deg0,
        }
    }

    /// Get rotation angle in degrees for logging
    pub fn degrees(self) -> u16 {
        match self {
            Rotation::Deg0 => 0,
            Rotation::Deg90 => 90,
            Rotation::Deg180 => 180,
            Rotation::Deg270 => 270,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Config {
    pub rgb: bool,
    pub inverted: bool,
    pub orientation: Orientation,
    pub height: u16,
    pub width: u16,
    pub dx: u16,
    pub dy: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rgb: false,
            inverted: false,
            orientation: Orientation::Landscape,
            height: 172,
            width: 320,
            dx: 0,
            dy: 34,
        }
    }
}

#[derive(Debug)]
pub enum Error<E = ()> {
    /// Communication error
    Comm(E),
    /// Pin setting error
    Pin(Infallible),
}

pub struct GC9307C<'b, SPI, DC, RST, TIMER>
where
    SPI: SpiDevice,
    DC: OutputPin<Error = Infallible>,
    RST: OutputPin<Error = Infallible>,
    TIMER: Timer,
{
    spi: SPI,
    dc: DC,
    rst: RST,
    config: Config,
    buffer: &'b mut [u8],
    _timer: core::marker::PhantomData<TIMER>,
    #[cfg(feature = "software-rotation")]
    current_rotation: Rotation,
    #[cfg(feature = "software-rotation")]
    logical_width: u16,
    #[cfg(feature = "software-rotation")]
    logical_height: u16,
}

#[maybe_async_cfg::maybe(
    sync(cfg(not(feature = "async")), self = "GC9307C",),
    async(feature = "async", keep_self)
)]
impl<'b, SPI, DC, RST, E, TIMER> GC9307C<'b, SPI, DC, RST, TIMER>
where
    SPI: SpiDevice<Error = E>,
    DC: OutputPin<Error = Infallible>,
    RST: OutputPin<Error = Infallible>,
    TIMER: Timer,
{
    pub fn new(config: Config, spi: SPI, dc: DC, rst: RST, buffer: &'b mut [u8]) -> Self {
        Self {
            spi,
            dc,
            rst,
            config,
            buffer,
            _timer: core::marker::PhantomData,
            #[cfg(feature = "software-rotation")]
            current_rotation: Rotation::Deg0,
            #[cfg(feature = "software-rotation")]
            logical_width: config.width,
            #[cfg(feature = "software-rotation")]
            logical_height: config.height,
        }
    }

    pub async fn init(&mut self) -> Result<(), Error<E>> {
        // Hardware reset first
        self.reset().await?;

        // Complete initialization sequence from docs/1.47寸IPS初始化GC9307+HSD.txt
        // Enable extended register access
        self.write_command(0xfe, &[]).await?;
        self.write_command(0xef, &[]).await?;

        // Memory access control and pixel format
        self.write_command(0x36, &[0x48]).await?; // Memory access control
        self.write_command(0x3a, &[0x05]).await?; // 16-bit color

        // Power regulation settings (0x85-0x8F series)
        self.write_command(0x85, &[0xc0]).await?;
        self.write_command(0x86, &[0x98]).await?;
        self.write_command(0x87, &[0x28]).await?;
        self.write_command(0x89, &[0x33]).await?;
        self.write_command(0x8B, &[0x84]).await?;
        self.write_command(0x8D, &[0x3B]).await?;
        self.write_command(0x8E, &[0x0f]).await?;
        self.write_command(0x8F, &[0x70]).await?;

        // Frame rate control
        self.write_command(0xe8, &[0x13, 0x17]).await?;

        // Additional power settings
        self.write_command(0xec, &[0x57, 0x07, 0xff]).await?;
        self.write_command(0xed, &[0x18, 0x09]).await?;
        self.write_command(0xc9, &[0x10]).await?;

        // Extended register settings
        self.write_command(0xff, &[0x61]).await?;
        self.write_command(0x99, &[0x3A]).await?;
        self.write_command(0x9d, &[0x43]).await?;
        self.write_command(0x98, &[0x3e]).await?;
        self.write_command(0x9c, &[0x4b]).await?;

        // Gamma correction settings (complete sequence)
        self.write_command(0xF0, &[0x06, 0x08, 0x08, 0x06, 0x05, 0x1d])
            .await?;
        self.write_command(0xF2, &[0x00, 0x01, 0x09, 0x07, 0x04, 0x23])
            .await?;
        self.write_command(0xF1, &[0x3b, 0x68, 0x66, 0x36, 0x35, 0x2f])
            .await?;
        self.write_command(0xF3, &[0x37, 0x6a, 0x66, 0x37, 0x35, 0x35])
            .await?;

        // Additional display control registers
        self.write_command(0xFA, &[0x80, 0x0f]).await?;
        self.write_command(0xBE, &[0x11]).await?; // source bias
        self.write_command(0xCB, &[0x02]).await?;
        self.write_command(0xCD, &[0x22]).await?;
        self.write_command(0x9B, &[0xFF]).await?;

        // Tearing effect
        self.write_command(0x35, &[0x00]).await?;
        self.write_command(0x44, &[0x00, 0x0a]).await?;

        // Sleep out and display on
        self.write_command(0x11, &[]).await?; // Sleep out
        TIMER::delay_ms(200).await; // Wait 200ms

        self.write_command(0x29, &[]).await?; // Display on
        self.write_command(0x2c, &[]).await?; // Memory write

        // Set initial orientation
        self.set_orientation(self.config.orientation).await?;
        Ok(())
    }

    pub async fn reset(&mut self) -> Result<(), Error<E>> {
        self.rst.set_high().map_err(Error::Pin)?;
        TIMER::delay_ms(10).await;
        self.rst.set_low().map_err(Error::Pin)?;
        TIMER::delay_ms(10).await;
        self.rst.set_high().map_err(Error::Pin)?;
        TIMER::delay_ms(120).await; // Wait for reset to complete

        Ok(())
    }

    pub async fn set_orientation(&mut self, orientation: Orientation) -> Result<(), Error<E>> {
        if self.config.rgb {
            self.write_command(0x36, &[orientation as u8]).await?;
        } else {
            self.write_command(0x36, &[orientation as u8 | 0x08])
                .await?;
        }
        self.config.orientation = orientation;
        Ok(())
    }

    /// Write command with optional parameters
    async fn write_command(&mut self, cmd: u8, params: &[u8]) -> Result<(), Error<E>> {
        // Set DC low for command
        self.dc.set_low().map_err(Error::Pin)?;
        self.spi.write(&[cmd]).await.map_err(Error::Comm)?;

        // Write parameters if any
        if !params.is_empty() {
            self.dc.set_high().map_err(Error::Pin)?;
            self.spi.write(params).await.map_err(Error::Comm)?;
        }
        Ok(())
    }

    /// Write raw pixel data to display (data mode)
    async fn write_raw_data(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        self.dc.set_high().map_err(Error::Pin)?;
        self.spi.write(data).await.map_err(Error::Comm)
    }

    /// Fill entire screen with a single color (optimized batch implementation)
    pub async fn fill_screen(&mut self, color: Rgb565) -> Result<(), Error<E>> {
        #[cfg(feature = "software-rotation")]
        let (width, height) = (self.logical_width, self.logical_height);
        #[cfg(not(feature = "software-rotation"))]
        let (width, height) = (self.config.width, self.config.height);

        self.set_address_window(0, 0, width - 1, height - 1).await?;

        let color_raw = RawU16::from(color).into_inner();
        let color_bytes = color_raw.to_be_bytes(); // Use big-endian for correct color display

        // Calculate total pixels
        let total_pixels = self.config.width as u32 * self.config.height as u32;

        // Use batch transmission for better performance
        const BATCH_SIZE: usize = 512; // Send 512 pixels at a time
        let mut batch_buffer = [0u8; BATCH_SIZE * 2]; // 2 bytes per pixel

        // Fill batch buffer with color
        for i in 0..BATCH_SIZE {
            batch_buffer[i * 2] = color_bytes[0];
            batch_buffer[i * 2 + 1] = color_bytes[1];
        }

        // Send full batches
        let full_batches = total_pixels / BATCH_SIZE as u32;
        for _ in 0..full_batches {
            self.write_raw_data(&batch_buffer).await?;
        }

        // Send remaining pixels
        let remaining_pixels = (total_pixels % BATCH_SIZE as u32) as usize;
        if remaining_pixels > 0 {
            let remaining_bytes = remaining_pixels * 2;
            self.write_raw_data(&batch_buffer[..remaining_bytes])
                .await?;
        }

        Ok(())
    }

    /// Fill a rectangular area with a color (optimized batch implementation)
    pub async fn fill_rect(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
        color: Rgb565,
    ) -> Result<(), Error<E>> {
        #[cfg(feature = "software-rotation")]
        let (screen_width, screen_height) = (self.logical_width, self.logical_height);
        #[cfg(not(feature = "software-rotation"))]
        let (screen_width, screen_height) = (self.config.width, self.config.height);

        // Bounds checking
        if x >= screen_width || y >= screen_height {
            return Ok(()); // Outside screen bounds
        }

        let actual_width = width.min(screen_width - x);
        let actual_height = height.min(screen_height - y);

        if actual_width == 0 || actual_height == 0 {
            return Ok(()); // Nothing to draw
        }

        self.set_address_window(x, y, x + actual_width - 1, y + actual_height - 1)
            .await?;

        let color_raw = RawU16::from(color).into_inner();
        let color_bytes = color_raw.to_be_bytes();

        let total_pixels = actual_width as u32 * actual_height as u32;

        // Use batch transmission for better performance
        if total_pixels <= 256 {
            // Small rectangles: send directly
            for _ in 0..total_pixels {
                self.write_raw_data(&color_bytes).await?;
            }
        } else {
            // Large rectangles: use batch transmission
            const BATCH_SIZE: usize = 256; // Send 256 pixels at a time
            let mut batch_buffer = [0u8; BATCH_SIZE * 2]; // 2 bytes per pixel

            // Fill batch buffer with color
            for i in 0..BATCH_SIZE {
                batch_buffer[i * 2] = color_bytes[0];
                batch_buffer[i * 2 + 1] = color_bytes[1];
            }

            // Send full batches
            let full_batches = total_pixels / BATCH_SIZE as u32;
            for _ in 0..full_batches {
                self.write_raw_data(&batch_buffer).await?;
            }

            // Send remaining pixels
            let remaining_pixels = (total_pixels % BATCH_SIZE as u32) as usize;
            if remaining_pixels > 0 {
                let remaining_bytes = remaining_pixels * 2;
                self.write_raw_data(&batch_buffer[..remaining_bytes])
                    .await?;
            }
        }

        Ok(())
    }

    /// Sets the global offset of the displayed image
    pub fn set_offset(&mut self, dx: u16, dy: u16) {
        self.config.dx = dx;
        self.config.dy = dy;
    }

    /// Sets the address window for the display with software rotation support
    pub async fn set_address_window(
        &mut self,
        sx: u16,
        sy: u16,
        ex: u16,
        ey: u16,
    ) -> Result<(), Error<E>> {
        #[cfg(feature = "software-rotation")]
        {
            // Transform logical coordinates to physical coordinates
            let (phys_sx, phys_sy) = self.transform_coordinates(sx, sy);
            let (phys_ex, phys_ey) = self.transform_coordinates(ex, ey);

            // Ensure we have the correct min/max values
            let min_x = phys_sx.min(phys_ex);
            let max_x = phys_sx.max(phys_ex);
            let min_y = phys_sy.min(phys_ey);
            let max_y = phys_sy.max(phys_ey);

            // Apply display offset
            let sx_offset = min_x + self.config.dx;
            let sy_offset = min_y + self.config.dy;
            let ex_offset = max_x + self.config.dx;
            let ey_offset = max_y + self.config.dy;

            // Column address set (0x2A)
            self.write_command(
                0x2A,
                &[
                    (sx_offset >> 8) as u8,
                    (sx_offset & 0xFF) as u8,
                    (ex_offset >> 8) as u8,
                    (ex_offset & 0xFF) as u8,
                ],
            )
            .await?;

            // Page address set (0x2B)
            self.write_command(
                0x2B,
                &[
                    (sy_offset >> 8) as u8,
                    (sy_offset & 0xFF) as u8,
                    (ey_offset >> 8) as u8,
                    (ey_offset & 0xFF) as u8,
                ],
            )
            .await?;

            // Memory write command (0x2C)
            self.write_command(0x2C, &[]).await?;
        }

        #[cfg(not(feature = "software-rotation"))]
        {
            // Apply display offset
            let sx_offset = sx + self.config.dx;
            let sy_offset = sy + self.config.dy;
            let ex_offset = ex + self.config.dx;
            let ey_offset = ey + self.config.dy;

            // Column address set (0x2A)
            self.write_command(
                0x2A,
                &[
                    (sx_offset >> 8) as u8,
                    (sx_offset & 0xFF) as u8,
                    (ex_offset >> 8) as u8,
                    (ex_offset & 0xFF) as u8,
                ],
            )
            .await?;

            // Page address set (0x2B)
            self.write_command(
                0x2B,
                &[
                    (sy_offset >> 8) as u8,
                    (sy_offset & 0xFF) as u8,
                    (ey_offset >> 8) as u8,
                    (ey_offset & 0xFF) as u8,
                ],
            )
            .await?;

            // Memory write command (0x2C)
            self.write_command(0x2C, &[]).await?;
        }

        Ok(())
    }

    pub async fn fill_color(&mut self, color: Rgb565) -> Result<(), Error<E>> {
        self.set_address_window(0, 0, self.config.width - 1, self.config.height - 1)
            .await?;
        let color = RawU16::from(color).into_inner();
        for i in 0..720 {
            let bytes = color.to_le_bytes(); // 将u16转换为小端字节序的[u8; 2]
            self.buffer[i * 2 + 1] = bytes[0]; // 存储低字节
            self.buffer[i * 2] = bytes[1]; // 存储高字节
        }
        // Memory write command is already sent in set_address_window
        self.dc.set_high().map_err(Error::Pin)?;
        for _ in 0..self.config.height / 2 {
            self.spi
                .write(&self.buffer[..1440])
                .await
                .map_err(Error::Comm)?;
        }
        Ok(())
    }

    pub async fn write_area(
        &mut self,
        x: u16,
        y: u16,
        width: u16,
        data: &[u8],
        color: Rgb565,
        bg_color: Rgb565,
    ) -> Result<(), Error<E>> {
        let height = MAX_DATA_LEN as u16 / width
            + if MAX_DATA_LEN as u16 % width > 0 {
                1
            } else {
                0
            };

        self.set_address_window(x, y, x + width - 1, y + height - 1)
            .await?;
        // Memory write command is already sent in set_address_window
        self.dc.set_high().map_err(Error::Pin)?;
        let color = RawU16::from(color).into_inner();
        let bg_color = RawU16::from(bg_color).into_inner();
        let front_bytes = color.to_le_bytes();
        let back_bytes = bg_color.to_le_bytes();
        for (i, bits) in data.iter().enumerate() {
            for j in 0..8 {
                if *bits & (1 << (7 - j)) != 0 {
                    self.buffer[(i * 8 + j) * 2] = front_bytes[1];
                    self.buffer[(i * 8 + j) * 2 + 1] = front_bytes[0];
                } else {
                    self.buffer[(i * 8 + j) * 2] = back_bytes[1];
                    self.buffer[(i * 8 + j) * 2 + 1] = back_bytes[0];
                }
            }
        }

        self.spi
            .write(&self.buffer[..data.len() * 8 * 2])
            .await
            .map_err(Error::Comm)?;
        Ok(())
    }

    #[cfg(feature = "software-rotation")]
    /// Set the current rotation (software rotation feature)
    pub fn set_rotation(&mut self, rotation: Rotation) {
        self.current_rotation = rotation;

        // Update logical dimensions based on rotation
        match rotation {
            Rotation::Deg0 | Rotation::Deg180 => {
                self.logical_width = self.config.width;
                self.logical_height = self.config.height;
            }
            Rotation::Deg90 | Rotation::Deg270 => {
                self.logical_width = self.config.height;
                self.logical_height = self.config.width;
            }
        }
    }

    #[cfg(feature = "software-rotation")]
    /// Get current rotation
    pub fn rotation(&self) -> Rotation {
        self.current_rotation
    }

    #[cfg(feature = "software-rotation")]
    /// Get logical screen dimensions (after rotation)
    pub fn logical_dimensions(&self) -> (u16, u16) {
        (self.logical_width, self.logical_height)
    }

    #[cfg(feature = "software-rotation")]
    /// Transform logical coordinates to physical coordinates based on rotation
    fn transform_coordinates(&self, x: u16, y: u16) -> (u16, u16) {
        match self.current_rotation {
            Rotation::Deg0 => (x, y),
            Rotation::Deg90 => (self.logical_height - 1 - y, x),
            Rotation::Deg180 => (self.logical_width - 1 - x, self.logical_height - 1 - y),
            Rotation::Deg270 => (y, self.logical_width - 1 - x),
        }
    }

    #[cfg(feature = "software-rotation")]
    /// Transform a rectangle from logical coordinates to physical coordinates
    fn transform_rect(&self, x: u16, y: u16, width: u16, height: u16) -> (u16, u16, u16, u16) {
        let (x1, y1) = self.transform_coordinates(x, y);
        let (x2, y2) = self.transform_coordinates(x + width - 1, y + height - 1);

        let min_x = x1.min(x2);
        let max_x = x1.max(x2);
        let min_y = y1.min(y2);
        let max_y = y1.max(y2);

        (min_x, min_y, max_x - min_x + 1, max_y - min_y + 1)
    }

    /// Draw a single pixel (basic drawing primitive)
    pub async fn set_pixel(&mut self, x: u16, y: u16, color: Rgb565) -> Result<(), Error<E>> {
        if x >= self.config.width || y >= self.config.height {
            return Ok(()); // Outside bounds
        }

        self.set_address_window(x, y, x, y).await?;

        let color_raw = RawU16::from(color).into_inner();
        let color_bytes = color_raw.to_be_bytes();

        self.write_raw_data(&color_bytes).await
    }

    /// Draw a simple 12px digit (0-9) for angle display
    #[cfg(feature = "font-rendering")]
    pub async fn draw_digit(
        &mut self,
        x: u16,
        y: u16,
        digit: u8,
        color: Rgb565,
    ) -> Result<(), Error<E>> {
        if digit > 9 {
            return Ok(()); // Invalid digit
        }

        let font_data = get_digit_font_data(digit);

        // Draw 12x16 character
        for row in 0..16 {
            for col in 0..12 {
                let byte_index = row * 2 + (col / 8); // 2 bytes per row (12 bits)
                let bit_index = 7 - (col % 8);

                if byte_index < font_data.len() {
                    let pixel_on = (font_data[byte_index] >> bit_index) & 1 == 1;
                    if pixel_on {
                        let _ = self.set_pixel(x + col as u16, y + row as u16, color).await;
                    }
                }
            }
        }

        Ok(())
    }

    /// Draw angle text (e.g., "0°", "90°", "180°", "270°")
    #[cfg(feature = "font-rendering")]
    pub async fn draw_angle_text(
        &mut self,
        x: u16,
        y: u16,
        angle: u16,
        color: Rgb565,
    ) -> Result<(), Error<E>> {
        let mut current_x = x;

        // Draw digits
        if angle >= 100 {
            let hundreds = (angle / 100) as u8;
            self.draw_digit(current_x, y, hundreds, color).await?;
            current_x += 13; // 12px width + 1px spacing
        }

        if angle >= 10 {
            let tens = ((angle / 10) % 10) as u8;
            self.draw_digit(current_x, y, tens, color).await?;
            current_x += 13;
        }

        let ones = (angle % 10) as u8;
        self.draw_digit(current_x, y, ones, color).await?;
        current_x += 13;

        // Draw degree symbol (simplified as small circle)
        self.draw_degree_symbol(current_x, y, color).await?;

        Ok(())
    }

    /// Draw degree symbol (°)
    #[cfg(feature = "font-rendering")]
    async fn draw_degree_symbol(&mut self, x: u16, y: u16, color: Rgb565) -> Result<(), Error<E>> {
        // Draw a small 4x4 circle for degree symbol
        let circle_pixels = [
            (1, 0),
            (2, 0),
            (0, 1),
            (3, 1),
            (0, 2),
            (3, 2),
            (1, 3),
            (2, 3),
        ];

        for (dx, dy) in circle_pixels.iter() {
            let _ = self.set_pixel(x + dx, y + dy, color).await;
        }

        Ok(())
    }
}

#[cfg(feature = "font-rendering")]
/// Get font data for digits 0-9 (12x16 bitmap)
fn get_digit_font_data(digit: u8) -> &'static [u8] {
    match digit {
        0 => &[
            0x3F, 0xC0, 0x7F, 0xE0, 0xE0, 0x70, 0xC0, 0x30, 0xC0, 0x30, 0xC0, 0x30, 0xC0, 0x30,
            0xC0, 0x30, 0xC0, 0x30, 0xC0, 0x30, 0xC0, 0x30, 0xC0, 0x30, 0xE0, 0x70, 0x7F, 0xE0,
            0x3F, 0xC0, 0x00, 0x00,
        ],
        1 => &[
            0x0C, 0x00, 0x1C, 0x00, 0x3C, 0x00, 0x0C, 0x00, 0x0C, 0x00, 0x0C, 0x00, 0x0C, 0x00,
            0x0C, 0x00, 0x0C, 0x00, 0x0C, 0x00, 0x0C, 0x00, 0x0C, 0x00, 0x0C, 0x00, 0x3F, 0x00,
            0x3F, 0x00, 0x00, 0x00,
        ],
        2 => &[
            0x3F, 0xC0, 0x7F, 0xE0, 0xE0, 0x70, 0x00, 0x30, 0x00, 0x30, 0x00, 0x70, 0x00, 0xE0,
            0x01, 0xC0, 0x03, 0x80, 0x07, 0x00, 0x0E, 0x00, 0x1C, 0x00, 0x38, 0x00, 0x7F, 0xF0,
            0xFF, 0xF0, 0x00, 0x00,
        ],
        3 => &[
            0x3F, 0xC0, 0x7F, 0xE0, 0xE0, 0x70, 0x00, 0x30, 0x00, 0x30, 0x00, 0x70, 0x0F, 0xE0,
            0x0F, 0xE0, 0x00, 0x70, 0x00, 0x30, 0x00, 0x30, 0xE0, 0x70, 0x7F, 0xE0, 0x3F, 0xC0,
            0x00, 0x00, 0x00, 0x00,
        ],
        4 => &[
            0x01, 0xC0, 0x03, 0xC0, 0x07, 0xC0, 0x0D, 0xC0, 0x19, 0xC0, 0x31, 0xC0, 0x61, 0xC0,
            0xC1, 0xC0, 0xFF, 0xF0, 0xFF, 0xF0, 0x01, 0xC0, 0x01, 0xC0, 0x01, 0xC0, 0x01, 0xC0,
            0x01, 0xC0, 0x00, 0x00,
        ],
        5 => &[
            0xFF, 0xF0, 0xFF, 0xF0, 0xE0, 0x00, 0xE0, 0x00, 0xE0, 0x00, 0xE0, 0x00, 0xFF, 0xC0,
            0xFF, 0xE0, 0x00, 0x70, 0x00, 0x30, 0x00, 0x30, 0xE0, 0x70, 0x7F, 0xE0, 0x3F, 0xC0,
            0x00, 0x00, 0x00, 0x00,
        ],
        6 => &[
            0x1F, 0xC0, 0x3F, 0xE0, 0x70, 0x70, 0xE0, 0x00, 0xE0, 0x00, 0xE0, 0x00, 0xFF, 0xC0,
            0xFF, 0xE0, 0xE0, 0x70, 0xE0, 0x30, 0xE0, 0x30, 0x70, 0x70, 0x7F, 0xE0, 0x3F, 0xC0,
            0x00, 0x00, 0x00, 0x00,
        ],
        7 => &[
            0xFF, 0xF0, 0xFF, 0xF0, 0x00, 0x30, 0x00, 0x60, 0x00, 0xC0, 0x01, 0x80, 0x03, 0x00,
            0x06, 0x00, 0x0C, 0x00, 0x18, 0x00, 0x30, 0x00, 0x60, 0x00, 0xC0, 0x00, 0xC0, 0x00,
            0xC0, 0x00, 0x00, 0x00,
        ],
        8 => &[
            0x3F, 0xC0, 0x7F, 0xE0, 0xE0, 0x70, 0xE0, 0x70, 0xE0, 0x70, 0x70, 0xE0, 0x3F, 0xC0,
            0x7F, 0xE0, 0xE0, 0x70, 0xE0, 0x70, 0xE0, 0x70, 0xE0, 0x70, 0x7F, 0xE0, 0x3F, 0xC0,
            0x00, 0x00, 0x00, 0x00,
        ],
        9 => &[
            0x3F, 0xC0, 0x7F, 0xE0, 0xE0, 0x70, 0xC0, 0x30, 0xC0, 0x30, 0xE0, 0x70, 0x7F, 0xF0,
            0x3F, 0xF0, 0x00, 0x70, 0x00, 0x70, 0x00, 0x70, 0xE0, 0xE0, 0x7F, 0xC0, 0x3F, 0x80,
            0x00, 0x00, 0x00, 0x00,
        ],
        _ => &[0; 32], // Empty for invalid digits
    }
}

#[maybe_async_cfg::maybe(
    sync(cfg(not(feature = "async")), self = "Timer",),
    async(feature = "async", keep_self)
)]
/// Simplified timer trait for delay operations.
pub trait Timer {
    /// Delay for the specified number of milliseconds.
    async fn delay_ms(milliseconds: u64);
}
