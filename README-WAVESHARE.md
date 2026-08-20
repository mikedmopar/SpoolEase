# Waveshare ESP32-S3 Touch LCD 5 SpoolEase Target

This branch adds a buildable SpoolEase hardware target for the Waveshare ESP32-S3-Touch-LCD-5 5-inch 1024x600 board with 16 MB flash and 8 MB Octal PSRAM.

## Build

Use the board feature from `core/`:

```sh
ESP_HAL_CONFIG_PSRAM_MODE=octal cargo build --release --no-default-features --features="esp32s3,waveshare-esp32-s3-touch-lcd-5" --target-dir "./target/waveshare"
```

The helper script wraps the same feature set:

```sh
./cargowaveshare build --release
```

Do not use this target as a stock Console release channel. Hardware flashing is intentionally outside Phase 2.

## Board

- Board: Waveshare ESP32-S3-Touch-LCD-5, 5-inch 1024x600 variant
- MCU: ESP32-S3
- Flash: 16 MB
- PSRAM: 8 MB Octal PSRAM, configured for 80 MHz
- LCD architecture: RGB/DPI panel
- Official controller identifier: ST7262
- Touch: GT911 on I2C
- Expander: CH422G for LCD reset, touch reset, backlight, and SD-card CS behavior

## Display

Initial timing follows the official Waveshare ESP-IDF 09 LVGL v9 RGB example for the 1024x600 configuration:

| Setting | Value |
| --- | ---: |
| Resolution | 1024 x 600 |
| Pixel clock | 21 MHz |
| HSYNC pulse width | 30 |
| HSYNC back porch | 145 |
| HSYNC front porch | 170 |
| VSYNC pulse width | 2 |
| VSYNC back porch | 23 |
| VSYNC front porch | 12 |
| Pixel format | RGB565 |
| RGB data width | 16-bit |
| PCLK edge | falling / negative |
| DE | GPIO5 |
| DISP | none |

The first fallback timing from the official Arduino `5_B` board config is horizontal-only:

| Setting | Value |
| --- | ---: |
| HSYNC pulse width | 24 |
| HSYNC back porch | 160 |
| HSYNC front porch | 160 |

That fallback is documented only. The implementation uses one timing configuration at a time.

## GPIO Map

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
| Touch reset | CH422G pin 1 |
| Backlight | CH422G pin 2 |
| LCD reset | CH422G pin 3 |
| SD MOSI | 11 |
| SD CLK | 12 |
| SD MISO | 13 |
| SD CS | CH422G pin 4 in official examples |

## Current Status

Phase 2 creates a buildable firmware target. The display, CH422G, GT911, PSRAM framebuffer allocation, and Slint runner paths compile.

Not yet hardware validated:

- LCD reset and backlight behavior
- GT911 reset/address strap behavior
- RGB timing on physical panel
- SD-card CS handling through CH422G
- Touch coordinate orientation

PN532 and HX711 mappings remain provisional future-phase work and are not finalized by this target.
