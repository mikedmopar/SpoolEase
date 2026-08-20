use alloc::{boxed::Box, rc::Rc, string::String};
use core::{cell::RefCell, slice};

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::Timer;
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::{
    dma::{
        AnyGdmaChannel, BurstConfig, DmaChannelConvert, ExternalBurstConfig, InternalBurstConfig,
    },
    gpio::{Level, Output, OutputConfig},
    lcd_cam::{
        lcd::dpi::{Config as DpiConfig, Dpi, Format, FrameTiming},
        LcdCam,
    },
    peripherals::LCD_CAM,
    spi,
    time::Rate,
};

use crate::{
    backlight::BacklightDevice,
    ch422g::Ch422g,
    framework::Framework,
    gt9x_adapter::{Gt9xAdapter, Gt9xAdapterConfig, Jc8048w550cGt911},
    mk_static,
    rgb_display::{
        display_bounce_bytes, display_bounce_out_desc_count, display_m2m_desc_count,
        display_precomputed_dst_ptr_count, display_precomputed_src_ptr_count, FlushPolicy,
        FrameMode, RGBDisplayConfig, RGBDisplayDmaStorage, RGBDisplayDriver, RGBDisplayResources,
        RefillPolicy,
    },
    sdcard_spi::create_sdcard_spi_device_dma,
    slint_ext::McuWindow,
    touch::Touch,
    ui_loop::UiRenderBackend,
};

const DISP_W: usize = 1024;
const DISP_W_TOTAL: usize = 1369;
const DISP_H: usize = 600;
const DISP_H_TOTAL: usize = 637;
const DISP_BPP: usize = 2;
const DISP_ROWS: usize = 10;
const DISP_FRAME_BYTES: usize = DISP_W * DISP_H * DISP_BPP;
const DISP_PCLK_HZ: u32 = 21_000_000;

const HSYNC_WIDTH: usize = 30;
const HSYNC_BACK_PORCH: usize = 145;
const HSYNC_FRONT_PORCH: usize = 170;
const VSYNC_WIDTH: usize = 2;
const VSYNC_FRONT_PORCH: usize = 12;

const DISP_BOUNCE_BYTES: usize = display_bounce_bytes(DISP_W, DISP_BPP, DISP_ROWS);
const DISP_BOUNCE_OUT_DESC_COUNT: usize =
    display_bounce_out_desc_count(DISP_W, DISP_H, DISP_BPP, DISP_ROWS);
const DISP_M2M_DESC_COUNT: usize = display_m2m_desc_count(DISP_W, DISP_BPP, DISP_ROWS);
const DISP_PRECOMPUTED_SRC_PTR_COUNT: usize =
    display_precomputed_src_ptr_count(DISP_W, DISP_H, DISP_BPP, DISP_ROWS);
const DISP_PRECOMPUTED_DST_PTR_COUNT: usize =
    display_precomputed_dst_ptr_count(DISP_W, DISP_BPP, DISP_ROWS);

#[repr(align(128))]
struct AlignedLineBuffer([slint::platform::software_renderer::Rgb565Pixel; DISP_W]);

type DisplayDmaStorage = RGBDisplayDmaStorage<
    DISP_BOUNCE_BYTES,
    DISP_BOUNCE_OUT_DESC_COUNT,
    DISP_M2M_DESC_COUNT,
    DISP_PRECOMPUTED_SRC_PTR_COUNT,
    DISP_PRECOMPUTED_DST_PTR_COUNT,
>;

#[cfg(feature = "rgb-stats")]
#[embassy_executor::task]
async fn stats_task() {
    loop {
        let stats = crate::rgb_display::RGBDisplayDriver::take_stats();
        let wait_avg_us = if stats.wait_on_miss_wait_count == 0 {
            0
        } else {
            stats.wait_on_miss_wait_total_us / stats.wait_on_miss_wait_count as u64
        };
        let out_isr_avg_us = if stats.out_isr_count == 0 {
            0
        } else {
            stats.out_isr_total_us / stats.out_isr_count as u64
        };
        let in_isr_avg_us = if stats.in_isr_count == 0 {
            0
        } else {
            stats.in_isr_total_us / stats.in_isr_count as u64
        };
        let isr_total_us = stats.out_isr_total_us + stats.in_isr_total_us;
        let isr_count = stats.out_isr_count as u64 + stats.in_isr_count as u64;
        let isr_avg_us = if isr_count == 0 {
            0
        } else {
            isr_total_us / isr_count
        };
        #[cfg(feature = "rgb-wait-on-miss-done-hint-on")]
        info!(
            "display_stats/s out_eof_while_inflight={} pending_same_half_overwrite={} m2m_copy_start={} stale_window_tx={} wait_on_miss_out_first={} wait_on_miss_done_hint_first={} wait_total_us={} wait_avg_us={} wait_timeout_count={}",
            stats.out_eof_while_inflight_count,
            stats.pending_same_half_overwrite_count,
            stats.m2m_copy_start_count,
            stats.stale_window_tx_count,
            stats.wait_on_miss_out_first_count,
            stats.wait_on_miss_done_hint_first_count,
            stats.wait_on_miss_wait_total_us,
            wait_avg_us,
            stats.wait_on_miss_timeout_count,
        );
        #[cfg(not(feature = "rgb-wait-on-miss-done-hint-on"))]
        info!(
            "display_stats/s out_eof_while_inflight={} pending_same_half_overwrite={} m2m_copy_start={} stale_window_tx={} wait_total_us={} wait_avg_us={} wait_timeout_count={}",
            stats.out_eof_while_inflight_count,
            stats.pending_same_half_overwrite_count,
            stats.m2m_copy_start_count,
            stats.stale_window_tx_count,
            stats.wait_on_miss_wait_total_us,
            wait_avg_us,
            stats.wait_on_miss_timeout_count,
        );
        info!(
            "isr_stats/s isr_total_us={} isr_avg_us={} out_isr_total_us={} out_isr_avg_us={} in_isr_total_us={} in_isr_avg_us={}",
            isr_total_us,
            isr_avg_us,
            stats.out_isr_total_us,
            out_isr_avg_us,
            stats.in_isr_total_us,
            in_isr_avg_us
        );
        Timer::after_secs(10).await;
    }
}

pub struct WaveshareEsp32S3TouchLcd5RenderBackend {
    display: RGBDisplayDriver,
    line_buffer: &'static mut AlignedLineBuffer,
    // window: Rc<McuWindow>,
}

impl UiRenderBackend for WaveshareEsp32S3TouchLcd5RenderBackend {
    fn render(&mut self, renderer: &slint::platform::software_renderer::SoftwareRenderer) -> bool {
        if let Some(mut frame_guard) = self.display.acquire_writable_frame() {
            struct FrameLineBuffer<'a> {
                frame_buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
                line_buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel; DISP_W],
                stride: usize,
            }

            impl<'a> slint::platform::software_renderer::LineBufferProvider for FrameLineBuffer<'a> {
                type TargetPixel = slint::platform::software_renderer::Rgb565Pixel;

                fn process_line(
                    &mut self,
                    line: usize,
                    range: core::ops::Range<usize>,
                    render_fn: impl FnOnce(&mut [Self::TargetPixel]),
                ) {
                    let src = &mut self.line_buffer[range.clone()];
                    render_fn(src);

                    let line_begin = line * self.stride;
                    let dst_start = line_begin + range.start;
                    let dst_end = line_begin + range.end;
                    let dst = &mut self.frame_buffer[dst_start..dst_end];

                    unsafe {
                        core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), src.len());
                    }
                }
            }

            let frame = frame_guard.buffer_mut();
            let pixel_count = frame.len()
                / core::mem::size_of::<slint::platform::software_renderer::Rgb565Pixel>();
            let pixels: &mut [slint::platform::software_renderer::Rgb565Pixel] =
                unsafe { slice::from_raw_parts_mut(frame.as_mut_ptr() as *mut _, pixel_count) };

            renderer.render_by_line(FrameLineBuffer {
                frame_buffer: pixels,
                line_buffer: &mut self.line_buffer.0,
                stride: DISP_W,
            });
            frame_guard
                .present()
                .expect("Failed to present RGB display frame");
            // if self.double_buffering {
            //     self.window.request_redraw();
            // }
            true
        } else {
            false // can't draw now, so skip drawing and return nothing was drawn, this will make slint_ext release the ui_loop right after to draw again
                  // self.window.request_redraw();
        }
    }
}

pub struct WaveshareEsp32S3TouchLcd5Backlight;

impl WaveshareEsp32S3TouchLcd5Backlight {
    pub fn new() -> Self {
        Self
    }
}

impl BacklightDevice for WaveshareEsp32S3TouchLcd5Backlight {
    type Error = ();

    fn set_percent(&mut self, _percent: u8) -> Result<(), Self::Error> {
        Ok(())
    }
}

pub struct EspBackend {
    pub window: Rc<McuWindow>,
}

impl slint::platform::Platform for EspBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> core::time::Duration {
        let now = esp_hal::time::Instant::now();
        let duration = now.duration_since_epoch();
        core::time::Duration::from_micros(duration.as_micros())
    }
    fn debug_log(&self, arguments: core::fmt::Arguments) {
        debug!("{}", arguments);
    }
}

#[allow(non_snake_case)]
pub struct WaveshareEsp32S3TouchLcd5DisplayPeripherals<CHLCD, CHM2M, SPIM2M, P>
where
    CHLCD: esp_hal::dma::TxChannelFor<LCD_CAM<'static>> + 'static,
    CHM2M: DmaChannelConvert<AnyGdmaChannel<'static>> + 'static,
    SPIM2M: esp_hal::dma::DmaEligible + 'static,
    P: esp_hal::i2c::master::Instance + 'static,
{
    pub LCD_CAM: LCD_CAM<'static>,
    pub DMA_CH_DPI: CHLCD,
    pub DMA_CH_M2M: CHM2M,
    pub SPI_M2M: SPIM2M,
    pub I2Cx: P,

    pub GPIO0: esp_hal::peripherals::GPIO0<'static>,
    pub GPIO1: esp_hal::peripherals::GPIO1<'static>,
    pub GPIO2: esp_hal::peripherals::GPIO2<'static>,
    pub GPIO3: esp_hal::peripherals::GPIO3<'static>,
    pub GPIO4: esp_hal::peripherals::GPIO4<'static>,
    pub GPIO5: esp_hal::peripherals::GPIO5<'static>,
    pub GPIO7: esp_hal::peripherals::GPIO7<'static>,
    pub GPIO8: esp_hal::peripherals::GPIO8<'static>,
    pub GPIO9: esp_hal::peripherals::GPIO9<'static>,
    pub GPIO10: esp_hal::peripherals::GPIO10<'static>,
    pub GPIO14: esp_hal::peripherals::GPIO14<'static>,
    pub GPIO17: esp_hal::peripherals::GPIO17<'static>,
    pub GPIO18: esp_hal::peripherals::GPIO18<'static>,
    pub GPIO21: esp_hal::peripherals::GPIO21<'static>,
    pub GPIO38: esp_hal::peripherals::GPIO38<'static>,
    pub GPIO39: esp_hal::peripherals::GPIO39<'static>,
    pub GPIO40: esp_hal::peripherals::GPIO40<'static>,
    pub GPIO41: esp_hal::peripherals::GPIO41<'static>,
    pub GPIO42: esp_hal::peripherals::GPIO42<'static>,
    pub GPIO45: esp_hal::peripherals::GPIO45<'static>,
    pub GPIO46: esp_hal::peripherals::GPIO46<'static>,
    pub GPIO47: esp_hal::peripherals::GPIO47<'static>,
    pub GPIO48: esp_hal::peripherals::GPIO48<'static>,
}

#[allow(non_snake_case)]
pub struct WaveshareEsp32S3TouchLcd5SDCardPeripherals<S, CHSD>
where
    S: esp_hal::spi::master::Instance + 'static,
    CHSD: esp_hal::dma::DmaChannelFor<spi::master::AnySpi<'static>>,
{
    pub GPIO15: esp_hal::peripherals::GPIO15<'static>,
    pub GPIO11: esp_hal::peripherals::GPIO11<'static>,
    pub GPIO12: esp_hal::peripherals::GPIO12<'static>,
    pub GPIO13: esp_hal::peripherals::GPIO13<'static>,
    pub SPIx: S,
    pub DMA_CHx: CHSD,
}

type InitDone = Signal<CriticalSectionRawMutex, Result<(), String>>;

pub enum WaveshareEsp32S3TouchLcd5FrameBuffers {
    Single(&'static mut [u8]),
    Double(&'static mut [u8], &'static mut [u8]),
}

pub struct WaveshareEsp32S3TouchLcd5 {
    init_done: &'static InitDone,
}

impl WaveshareEsp32S3TouchLcd5 {
    #[allow(clippy::type_complexity)]
    pub fn new<'a, CHLCD, CHM2M, SPIM2M, P, S, CHSD>(
        display_peripherals: WaveshareEsp32S3TouchLcd5DisplayPeripherals<CHLCD, CHM2M, SPIM2M, P>,
        sdcard_peripherals: WaveshareEsp32S3TouchLcd5SDCardPeripherals<S, CHSD>,
        frame_buffers: WaveshareEsp32S3TouchLcd5FrameBuffers,
        touch_config: Gt9xAdapterConfig,
        framework: Rc<RefCell<Framework>>,
    ) -> (
        Self,
        WaveshareEsp32S3TouchLcd5Runner<CHLCD, CHM2M, SPIM2M, P>,
        ExclusiveDevice<
            esp_hal::spi::master::SpiDmaBus<'static, esp_hal::Async>,
            esp_hal::gpio::Output<'a>,
            embedded_hal_bus::spi::NoDelay,
        >,
    )
    where
        CHLCD: esp_hal::dma::TxChannelFor<LCD_CAM<'static>> + 'static,
        CHM2M: DmaChannelConvert<AnyGdmaChannel<'static>> + 'static,
        SPIM2M: esp_hal::dma::DmaEligible + 'static,
        P: esp_hal::i2c::master::Instance + 'static,
        S: esp_hal::spi::master::Instance + 'static,
        CHSD: esp_hal::dma::DmaChannelFor<spi::master::AnySpi<'static>> + 'a + 'static,
    {
        let init_done = mk_static!(InitDone, InitDone::new());
        let runner = WaveshareEsp32S3TouchLcd5Runner {
            peripherals: Some(display_peripherals),
            frame_buffers: Some(frame_buffers),
            touch_config,
            framework,
            init_done,
        };
        let me = Self { init_done };

        let sd_cs = Output::new(
            sdcard_peripherals.GPIO15,
            Level::High,
            OutputConfig::default(),
        );
        let sd_sclk = sdcard_peripherals.GPIO12;
        let sd_miso = sdcard_peripherals.GPIO13;
        let sd_mosi = sdcard_peripherals.GPIO11;

        let sdcard_spi_device = create_sdcard_spi_device_dma(
            sdcard_peripherals.SPIx,
            sdcard_peripherals.DMA_CHx,
            sd_cs,
            sd_sclk,
            sd_miso,
            sd_mosi,
            Rate::from_mhz(20),
        );

        (me, runner, sdcard_spi_device)
    }

    pub async fn wait_init_done(&self) -> Result<(), String> {
        self.init_done.wait().await
    }
}

pub struct WaveshareEsp32S3TouchLcd5Runner<CHLCD, CHM2M, SPIM2M, P>
where
    CHLCD: esp_hal::dma::TxChannelFor<LCD_CAM<'static>> + 'static,
    CHM2M: DmaChannelConvert<AnyGdmaChannel<'static>> + 'static,
    SPIM2M: esp_hal::dma::DmaEligible + 'static,
    P: esp_hal::i2c::master::Instance + 'static,
{
    peripherals: Option<WaveshareEsp32S3TouchLcd5DisplayPeripherals<CHLCD, CHM2M, SPIM2M, P>>,
    frame_buffers: Option<WaveshareEsp32S3TouchLcd5FrameBuffers>,
    touch_config: Gt9xAdapterConfig,
    framework: Rc<RefCell<Framework>>,
    init_done: &'static InitDone,
}

impl<CHLCD, CHM2M, SPIM2M, P> WaveshareEsp32S3TouchLcd5Runner<CHLCD, CHM2M, SPIM2M, P>
where
    CHLCD: esp_hal::dma::TxChannelFor<LCD_CAM<'static>> + 'static,
    CHM2M: DmaChannelConvert<AnyGdmaChannel<'static>> + 'static,
    SPIM2M: esp_hal::dma::DmaEligible + 'static,
    P: esp_hal::i2c::master::Instance + 'static,
{
    pub async fn run(&mut self) {
        let peripherals = self
            .peripherals
            .take()
            .expect("Display peripherals missing");
        let frame_buffers = self
            .frame_buffers
            .take()
            .expect("Display frame buffer missing");

        let lcd_cam = LcdCam::new(peripherals.LCD_CAM);
        let dpi_cfg = DpiConfig::default()
            .with_clock_mode(esp_hal::lcd_cam::lcd::ClockMode {
                polarity: esp_hal::lcd_cam::lcd::Polarity::IdleLow,
                phase: esp_hal::lcd_cam::lcd::Phase::ShiftHigh,
            })
            .with_frequency(Rate::from_hz(DISP_PCLK_HZ))
            .with_format(Format {
                enable_2byte_mode: true,
                ..Default::default()
            })
            .with_timing(FrameTiming {
                horizontal_active_width: DISP_W,
                horizontal_total_width: DISP_W_TOTAL,
                horizontal_blank_front_porch: HSYNC_FRONT_PORCH,
                vertical_active_height: DISP_H,
                vertical_total_height: DISP_H_TOTAL,
                vertical_blank_front_porch: VSYNC_FRONT_PORCH,
                hsync_width: HSYNC_WIDTH,
                vsync_width: VSYNC_WIDTH,
                hsync_position: HSYNC_BACK_PORCH,
            })
            .with_vsync_idle_level(Level::Low)
            .with_hsync_idle_level(Level::Low)
            .with_de_idle_level(Level::Low)
            .with_disable_black_region(false);

        let dpi: Dpi<'static, esp_hal::Blocking> =
            Dpi::new(lcd_cam.lcd, peripherals.DMA_CH_DPI, dpi_cfg)
                .unwrap()
                .with_vsync(peripherals.GPIO3)
                .with_hsync(peripherals.GPIO46)
                .with_de(peripherals.GPIO5)
                .with_pclk(peripherals.GPIO7)
                .with_data0(peripherals.GPIO14)
                .with_data1(peripherals.GPIO38)
                .with_data2(peripherals.GPIO18)
                .with_data3(peripherals.GPIO17)
                .with_data4(peripherals.GPIO10)
                .with_data5(peripherals.GPIO39)
                .with_data6(peripherals.GPIO0)
                .with_data7(peripherals.GPIO45)
                .with_data8(peripherals.GPIO48)
                .with_data9(peripherals.GPIO47)
                .with_data10(peripherals.GPIO21)
                .with_data11(peripherals.GPIO1)
                .with_data12(peripherals.GPIO2)
                .with_data13(peripherals.GPIO42)
                .with_data14(peripherals.GPIO41)
                .with_data15(peripherals.GPIO40);

        let (frame_mode, repaint_buffer_type, frames): (
            FrameMode,
            slint::platform::software_renderer::RepaintBufferType,
            &'static mut [&'static mut [u8]],
        ) = match frame_buffers {
            WaveshareEsp32S3TouchLcd5FrameBuffers::Single(frame_buffer) => {
                assert!(
                    frame_buffer.len() == DISP_FRAME_BYTES,
                    "Frame buffer length mismatch: expected {} bytes, got {}",
                    DISP_FRAME_BYTES,
                    frame_buffer.len()
                );
                (
                    FrameMode::SingleBuffer,
                    slint::platform::software_renderer::RepaintBufferType::ReusedBuffer,
                    Box::leak(Box::new([frame_buffer])),
                )
            }
            WaveshareEsp32S3TouchLcd5FrameBuffers::Double(frame_buffer_a, frame_buffer_b) => {
                assert!(
                    frame_buffer_a.len() == DISP_FRAME_BYTES,
                    "Frame buffer A length mismatch: expected {} bytes, got {}",
                    DISP_FRAME_BYTES,
                    frame_buffer_a.len()
                );
                assert!(
                    frame_buffer_b.len() == DISP_FRAME_BYTES,
                    "Frame buffer B length mismatch: expected {} bytes, got {}",
                    DISP_FRAME_BYTES,
                    frame_buffer_b.len()
                );
                (
                    FrameMode::DoubleBuffering,
                    slint::platform::software_renderer::RepaintBufferType::SwappedBuffers,
                    Box::leak(Box::new([frame_buffer_a, frame_buffer_b])),
                )
            }
        };

        let cfg = RGBDisplayConfig {
            width: DISP_W,
            height: DISP_H,
            bytes_per_pixel: DISP_BPP,
            rows_per_window: DISP_ROWS,
            pixel_clock_hz: DISP_PCLK_HZ,
            horizontal_total_width: DISP_W_TOTAL as u32,
            vertical_total_height: DISP_H_TOTAL as u32,
            burst: BurstConfig {
                internal_memory: InternalBurstConfig::Enabled,
                external_memory: ExternalBurstConfig::Size64,
            },
            flush: FlushPolicy::Enabled,
            refill_policy: RefillPolicy::WaitOnMiss,
            frame_mode,
        };

        let display_dma_storage = mk_static!(DisplayDmaStorage, DisplayDmaStorage::new());
        let dma_storage = display_dma_storage.as_storage_mut();

        let display_resources = RGBDisplayResources {
            dpi,
            dma: peripherals.DMA_CH_M2M,
            spi: peripherals.SPI_M2M,
            frames,
        };

        let mut display = RGBDisplayDriver::new(cfg, dma_storage, display_resources)
            .expect("Failed to create RGB display driver");
        display.start().expect("Failed to start RGB display driver");

        #[cfg(feature = "rgb-stats")]
        self.framework.borrow().spawner.spawn(stats_task()).ok();

        let window = McuWindow::new(repaint_buffer_type);
        window.set_size(slint::PhysicalSize::new(DISP_W as u32, DISP_H as u32));
        self.framework
            .borrow_mut()
            .set_display_window(window.clone());
        slint::platform::set_platform(Box::new(EspBackend {
            window: window.clone(),
        }))
        .expect("backend already initialized");

        let touch_int_strap =
            Output::new(peripherals.GPIO4, Level::Low, OutputConfig::default());

        let i2c = esp_hal::i2c::master::I2c::new(
            peripherals.I2Cx,
            esp_hal::i2c::master::Config::default().with_frequency(Rate::from_khz(400)),
        )
        .unwrap()
        .with_sda(peripherals.GPIO8)
        .with_scl(peripherals.GPIO9)
        .into_async();

        let mut ch422g = Ch422g::new(i2c);
        ch422g
            .enable_output_mode()
            .await
            .expect("Failed to enable CH422G output mode");

        // Official Waveshare examples hold GT911 INT low while the CH422G
        // asserts and releases the touch reset output. That strap selects the
        // controller address used by the gt9x driver before GPIO4 is released.
        ch422g
            .assert_touch_reset_state()
            .await
            .expect("Failed to assert GT911 reset through CH422G");
        Timer::after_millis(100).await;
        ch422g
            .release_touch_reset_state()
            .await
            .expect("Failed to release GT911 reset through CH422G");
        Timer::after_millis(200).await;
        ch422g
            .enable_backlight_state()
            .await
            .expect("Failed to enable Waveshare backlight through CH422G");
        drop(touch_int_strap);

        let touch_i2c = ch422g.release();

        let mut touch_buf = [0u8; 64];
        let mut touch_inner: gt9x::Gt9x<Jc8048w550cGt911, _, _, _, _> =
            gt9x::Gt9x::new(touch_i2c, &mut touch_buf);
        touch_inner
            .init()
            .await
            .expect("Failed to initialize GT9x touch controller");

        let touch = Touch::new(Gt9xAdapter::new(touch_inner, self.touch_config));
        let line_buffer = mk_static!(
            AlignedLineBuffer,
            AlignedLineBuffer([slint::platform::software_renderer::Rgb565Pixel(0); DISP_W])
        );
        let render_backend = WaveshareEsp32S3TouchLcd5RenderBackend {
            display,
            line_buffer,
            // window: window.clone(),
        };
        let mut backlight = WaveshareEsp32S3TouchLcd5Backlight::new();

        backlight
            .set_percent(100)
            .expect("Failed to set display backlight to 100%");

        self.init_done.signal(Ok(()));

        crate::ui_loop::event_loop(
            touch,
            window,
            render_backend,
            backlight,
            self.framework.clone(),
        )
        .await;
    }
}
