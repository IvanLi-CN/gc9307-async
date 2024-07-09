#![no_std]

use core::convert::Infallible;
use core::future::Future;
use core::marker::PhantomData;

use embedded_graphics_core::pixelcolor::{raw::RawU16, Rgb565};
use embedded_graphics_core::prelude::RawData;
use embedded_hal::digital::OutputPin;
#[cfg(not(feature = "async"))]
use embedded_hal::spi::SpiDevice;
#[cfg(feature = "async")]
use embedded_hal_async::spi::SpiDevice;

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
    Portrait = 0x00,
    Landscape = 0x20,
    PortraitSwapped = 0xC0,
    LandscapeSwapped = 0xA0,
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
    _timer: PhantomData<TIMER>,
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
            _timer: PhantomData,
        }
    }

    pub async fn init(&mut self) -> Result<(), Error<E>> {
        self.reset().await?;

        let dc = &mut self.dc;

        struct Command<'a> {
            instruction: Instruction,
            params: &'a [u8],
            delay_time: u64,
        }

        impl<'a> Command<'a> {
            fn new(instruction: Instruction, params: &'a [u8], delay_time: u64) -> Self {
                Self {
                    instruction,
                    params,
                    delay_time,
                }
            }
        }

        let commands = [
            // Enable extended register access
            Command::new(Instruction::ExtendedRegAccess1, &[], 0), // Enable Level 1 commands
            Command::new(Instruction::ExtendedRegAccess2, &[], 0), // Enable Level 2 commands
            // Memory and interface configuration
            Command::new(Instruction::MemoryAccessControl, &[0x48], 0), // Set orientation/BGR order
            Command::new(Instruction::PixelFormatSet, &[0x05], 0),      // RGB565 format (16-bit)
            // Power regulation settings
            Command::new(Instruction::VcoreVoltageControl, &[0x28], 0), // Core voltage adjustment
            Command::new(Instruction::Vreg1aControl, &[0xC0], 0),       // VREG1A = 0xC0
            Command::new(Instruction::Vreg1bControl, &[0x98], 0),       // VREG1B = 0x98
            Command::new(Instruction::Vreg2aControl, &[0x10], 0),       // VREG2A = 0x10
            // Timing configuration
            Command::new(Instruction::FrameRateControl, &[0x13, 0x17], 0), // Frame rate 70Hz
            Command::new(Instruction::BlankingPorchControl, &[0x33], 0),   // Vertical blanking
            Command::new(Instruction::DisplayFunctionControl, &[0x3A], 0), // Scan direction config
            // Gamma correction
            Command::new(
                Instruction::GammaSet1,
                &[0x06, 0x08, 0x08, 0x06, 0x05, 0x1D],
                0,
            ),
            Command::new(
                Instruction::GammaSet2,
                &[0x3B, 0x68, 0x66, 0x36, 0x35, 0x2F],
                0,
            ),
            Command::new(
                Instruction::GammaSet3,
                &[0x00, 0x01, 0x09, 0x07, 0x04, 0x23],
                0,
            ),
            Command::new(
                Instruction::GammaSet4,
                &[0x37, 0x6A, 0x66, 0x37, 0x35, 0x35],
                0,
            ),
            // Display control
            Command::new(Instruction::TearingEffectEnable, &[0x00], 0), // Enable TE output
            Command::new(Instruction::SleepOut, &[], 120),              // Exit sleep, 120ms delay
            Command::new(Instruction::DisplayOn, &[], 20),              // Enable display
        ];

        for Command {
            instruction,
            params,
            delay_time,
        } in commands
        {
            dc.set_low().ok();
            let mut data = [0_u8; 1];
            data.copy_from_slice(&[instruction as u8]);
            self.spi.write(&data).await.map_err(Error::Comm)?;
            if !params.is_empty() {
                dc.set_high().ok();
                let mut buf = [0_u8; 8];
                buf[..params.len()].copy_from_slice(params);
                self.spi
                    .write(&buf[..params.len()])
                    .await
                    .map_err(Error::Comm)?;
            }
            if delay_time > 0 {
                TIMER::after_millis(delay_time).await;
            }
        }

        self.set_orientation(self.config.orientation).await?;
        Ok(())
    }

    pub async fn reset(&mut self) -> Result<(), Error<E>> {
        self.rst.set_high().map_err(Error::Pin)?;
        TIMER::after_millis(10).await;
        self.rst.set_low().map_err(Error::Pin)?;
        TIMER::after_millis(10).await;
        self.rst.set_high().map_err(Error::Pin)?;

        Ok(())
    }

    pub async fn set_orientation(&mut self, orientation: Orientation) -> Result<(), Error<E>> {
        if self.config.rgb {
            self.write_command(Instruction::MemoryAccessControl, &[orientation as u8])
                .await?;
        } else {
            self.write_command(
                Instruction::MemoryAccessControl,
                &[orientation as u8 | 0x08],
            )
            .await?;
        }
        self.config.orientation = orientation;
        Ok(())
    }

    async fn write_command(
        &mut self,
        instruction: Instruction,
        params: &[u8],
    ) -> Result<(), Error<E>> {
        let dc = &mut self.dc;
        dc.set_low().ok();
        let mut data = [0_u8; 1];
        data.copy_from_slice(&[instruction as u8]);
        self.spi.write(&data).await.map_err(Error::Comm)?;
        if !params.is_empty() {
            dc.set_high().ok();
            let mut buf = [0_u8; 8];
            buf[..params.len()].copy_from_slice(params);
            self.spi
                .write(&buf[..params.len()])
                .await
                .map_err(Error::Comm)?;
        }
        Ok(())
    }

    fn start_data(&mut self) -> Result<(), Error<E>> {
        self.dc.set_high().map_err(Error::Pin)
    }

    async fn write_data(&mut self, data: &[u8]) -> Result<(), Error<E>> {
        let mut buf = [0_u8; 8];
        buf[..data.len()].copy_from_slice(data);
        self.spi
            .write(&buf[..data.len()])
            .await
            .map_err(Error::Comm)
    }

    /// Sets the global offset of the displayed image
    pub fn set_offset(&mut self, dx: u16, dy: u16) {
        self.config.dx = dx;
        self.config.dy = dy;
    }

    /// Sets the address window for the display.
    pub async fn set_address_window(
        &mut self,
        sx: u16,
        sy: u16,
        ex: u16,
        ey: u16,
    ) -> Result<(), Error<E>> {
        self.write_command(Instruction::ColumnAddressSet, &[])
            .await?;
        self.start_data()?;
        let sx_bytes = (sx + self.config.dx).to_be_bytes();
        let ex_bytes = (ex + self.config.dx).to_be_bytes();
        self.write_data(&[sx_bytes[0], sx_bytes[1], ex_bytes[0], ex_bytes[1]])
            .await?;
        self.write_command(Instruction::PageAddressSet, &[]).await?;
        self.start_data()?;
        let sy_bytes = (sy + self.config.dy).to_be_bytes();
        let ey_bytes = (ey + self.config.dy).to_be_bytes();
        self.write_data(&[sy_bytes[0], sy_bytes[1], ey_bytes[0], ey_bytes[1]])
            .await
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
        self.write_command(Instruction::MemoryWrite, &[]).await?;
        self.start_data()?;
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
        self.write_command(Instruction::MemoryWrite, &[]).await?;
        self.start_data()?;
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
}

/// The timer trait to implement by the user application.
pub trait Timer {
    /// Expire after the specified number of milliseconds.
    fn after_millis(milliseconds: u64) -> impl Future<Output = ()>;
}
