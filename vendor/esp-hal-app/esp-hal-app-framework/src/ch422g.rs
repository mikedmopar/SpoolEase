use embedded_hal_async::i2c::I2c;
use core::convert::Infallible;

pub const CH422G_SYSTEM_ADDR: u8 = 0x24;
pub const CH422G_OUTPUT_ADDR: u8 = 0x38;

const SYSTEM_OUTPUT_ENABLE: u8 = 0x01;

// Values copied from the official Waveshare ESP-IDF board port for
// ESP32-S3-Touch-LCD-5. The SD-only example uses full-byte writes that can
// change unrelated outputs, so runtime updates below preserve a shadow state.
const OUTPUT_TOUCH_RESET_ASSERTED: u8 = 0x2c;
const OUTPUT_TOUCH_RESET_RELEASED: u8 = 0x2e;
const OUTPUT_BACKLIGHT_ON: u8 = 0x1e;

const EXIO4_SD_CS: u8 = 1 << 4;

pub struct Ch422g<I2C> {
    i2c: I2C,
    output_state: u8,
}

impl<I2C> Ch422g<I2C>
where
    I2C: I2c,
{
    pub fn new(i2c: I2C) -> Self {
        Self {
            i2c,
            output_state: OUTPUT_TOUCH_RESET_ASSERTED,
        }
    }

    pub async fn enable_output_mode(&mut self) -> Result<(), I2C::Error> {
        self.write_state(CH422G_SYSTEM_ADDR, SYSTEM_OUTPUT_ENABLE).await
    }

    pub async fn assert_touch_reset_state(&mut self) -> Result<(), I2C::Error> {
        self.write_output_state(OUTPUT_TOUCH_RESET_ASSERTED).await
    }

    pub async fn release_touch_reset_state(&mut self) -> Result<(), I2C::Error> {
        self.write_output_state(OUTPUT_TOUCH_RESET_RELEASED).await
    }

    pub async fn enable_backlight_state(&mut self) -> Result<(), I2C::Error> {
        self.write_output_state(OUTPUT_BACKLIGHT_ON).await
    }

    pub async fn select_sd_card_state(&mut self) -> Result<(), I2C::Error> {
        self.set_output_bit(EXIO4_SD_CS, false).await
    }

    pub async fn deselect_sd_card_state(&mut self) -> Result<(), I2C::Error> {
        self.set_output_bit(EXIO4_SD_CS, true).await
    }

    pub fn release(self) -> I2C {
        self.i2c
    }

    async fn set_output_bit(&mut self, mask: u8, high: bool) -> Result<(), I2C::Error> {
        let output_state = if high {
            self.output_state | mask
        } else {
            self.output_state & !mask
        };
        self.write_output_state(output_state).await
    }

    async fn write_output_state(&mut self, value: u8) -> Result<(), I2C::Error> {
        self.write_state(CH422G_OUTPUT_ADDR, value).await?;
        self.output_state = value;
        Ok(())
    }

    async fn write_state(&mut self, address: u8, value: u8) -> Result<(), I2C::Error> {
        self.i2c.write(address, &[value]).await
    }
}

pub struct WaveshareSdCardCs;

impl embedded_hal::digital::ErrorType for WaveshareSdCardCs {
    type Error = Infallible;
}

impl embedded_hal::digital::OutputPin for WaveshareSdCardCs {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
