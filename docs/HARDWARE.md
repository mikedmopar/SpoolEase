# Hardware Notes

## Waveshare ESP32-S3-Touch-LCD-5

The `waveshare-esp32-s3-touch-lcd-5` feature targets the Waveshare 5-inch 1024x600 ESP32-S3 board with 16 MB flash and 8 MB Octal PSRAM.

The LCD is treated as a 16-bit RGB565 RGB/DPI panel. The exact official Waveshare board configuration identifies the LCD controller as ST7262. ST7701 appears in older/generic Waveshare README material but is not used as the implementation model for this target.

### Core Devices

| Device | Role |
| --- | --- |
| ST7262 RGB panel config | 1024x600 RGB/DPI LCD |
| GT911 | Capacitive touch controller |
| CH422G | I/O expander for reset/backlight/SD CS behavior |
| PSRAM | External framebuffers for RGB display |

### RGB Timing

| Setting | Active Value |
| --- | ---: |
| Pixel clock | 21 MHz |
| Width | 1024 |
| Height | 600 |
| HSYNC pulse | 30 |
| HSYNC back porch | 145 |
| HSYNC front porch | 170 |
| VSYNC pulse | 2 |
| VSYNC back porch | 23 |
| VSYNC front porch | 12 |
| RGB width | 16-bit |
| Pixel format | RGB565 |

Fallback timing documented from the official Arduino 5_B config: `HPW=24`, `HBP=160`, `HFP=160`. It is not active in code.

### GPIOs

| Function | GPIO |
| --- | ---: |
| I2C SDA | 8 |
| I2C SCL | 9 |
| VSYNC | 3 |
| HSYNC | 46 |
| DE | 5 |
| PCLK | 7 |
| RGB D0 | 14 |
| RGB D1 | 38 |
| RGB D2 | 18 |
| RGB D3 | 17 |
| RGB D4 | 10 |
| RGB D5 | 39 |
| RGB D6 | 0 |
| RGB D7 | 45 |
| RGB D8 | 48 |
| RGB D9 | 47 |
| RGB D10 | 21 |
| RGB D11 | 1 |
| RGB D12 | 2 |
| RGB D13 | 42 |
| RGB D14 | 41 |
| RGB D15 | 40 |
| Touch INT strap | 4 |
| SD MOSI | 11 |
| SD CLK | 12 |
| SD MISO | 13 |

### Implementation Status

The Phase 2 code is compile-only. No hardware behavior has been verified yet. CH422G output states are based on official Waveshare examples and must be checked on first hardware boot, especially SD-card CS behavior because SpoolEase currently expects a GPIO-backed SPI CS device.

PN532 and HX711 pins are not finalized in Phase 2.
