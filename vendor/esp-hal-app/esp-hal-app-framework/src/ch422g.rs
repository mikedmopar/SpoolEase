use embedded_hal_async::i2c::I2c;

pub const CH422G_SYSTEM_ADDR: u8 = 0x24;
pub const CH422G_OUTPUT_ADDR: u8 = 0x38;

const SYSTEM_OUTPUT_ENABLE: u8 = 0x01;

// Values copied from the official Waveshare ESP-IDF board port for
// ESP32-S3-Touch-LCD-5. They represent complete expander output states, not
// individual register updates, so keep them centralized and named.
const OUTPUT_TOUCH_RESET_ASSERTED: u8 = 0x2c;
const OUTPUT_TOUCH_RESET_RELEASED: u8 = 0x2e;
const OUTPUT_BACKLIGHT_ON: u8 = 0x1e;
const OUTPUT_SD_SELECTED: u8 = 0x0a;

pub struct Ch422g<I2C> {
    i2c: I2C,
}

impl<I2C> Ch422g<I2C>
where
    I2C: I2c,
{
    pub fn new(i2c: I2C) -> Self {
        Self { i2c }
    }

    pub async fn enable_output_mode(&mut self) -> Result<(), I2C::Error> {
        self.write_state(CH422G_SYSTEM_ADDR, SYSTEM_OUTPUT_ENABLE).await
    }

    pub async fn assert_touch_reset_state(&mut self) -> Result<(), I2C::Error> {
        self.write_state(CH422G_OUTPUT_ADDR, OUTPUT_TOUCH_RESET_ASSERTED)
            .await
    }

    pub async fn release_touch_reset_state(&mut self) -> Result<(), I2C::Error> {
        self.write_state(CH422G_OUTPUT_ADDR, OUTPUT_TOUCH_RESET_RELEASED)
            .await
    }

    pub async fn enable_backlight_state(&mut self) -> Result<(), I2C::Error> {
        self.write_state(CH422G_OUTPUT_ADDR, OUTPUT_BACKLIGHT_ON).await
    }

    pub async fn select_sd_card_state(&mut self) -> Result<(), I2C::Error> {
        self.write_state(CH422G_OUTPUT_ADDR, OUTPUT_SD_SELECTED).await
    }

    pub fn release(self) -> I2C {
        self.i2c
    }

    async fn write_state(&mut self, address: u8, value: u8) -> Result<(), I2C::Error> {
        self.i2c.write(address, &[value]).await
    }
}
