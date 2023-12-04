use cstr_core::CString;

use embedded_graphics_core::draw_target::DrawTarget;

use std::thread;
use std::time::Instant;

use display_interface_spi::SPIInterfaceNoCS;
use mipidsi::{Builder, ColorOrder, Orientation};

use esp_idf_hal::{
    delay,
    gpio::*,
    peripherals::Peripherals,
    spi::{config::DriverConfig, Dma, SpiConfig, SpiDeviceDriver},
    units::FromValueType, // for converting 26MHz to value
};

use lvgl::font::Font;
use lvgl::style::Style;
use lvgl::widgets::Label;
use lvgl::{Align, Color, Display, DrawBuffer, LvError, Part, TextAlign, Widget};

fn main() -> Result<(), LvError> {
    const HOR_RES: u32 = 320;
    const VER_RES: u32 = 240;
    const LINES: u32 = 20;

    // It is necessary to call this function once. Otherwise some patches to the runtime
    // implemented by esp-idf-sys might not link properly. See https://github.com/esp-rs/esp-idf-template/issues/71
    esp_idf_svc::sys::link_patches();

    // Bind the log crate to the ESP Logging facilities
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("================= Starting App =======================");

    let peripherals = Peripherals::take().unwrap();

    // #[allow(unused)]
    let pins = peripherals.pins;

    let spi = SpiDeviceDriver::new_single(
        peripherals.spi2,
        pins.gpio18,       // sclk
        pins.gpio23,       // sdo
        None::<AnyIOPin>,  // sdi
        Some(pins.gpio14), // cs
        &DriverConfig::new().dma(Dma::Channel1(4096)),
        &SpiConfig::new().write_only(true).baudrate(26.MHz().into()),
    )
    .unwrap();

    let rst = PinDriver::output(pins.gpio33).unwrap();
    let dc = PinDriver::output(pins.gpio27).unwrap();
    let di = SPIInterfaceNoCS::new(spi, dc);

    // Turn backlight on
    let mut bklt = PinDriver::output(pins.gpio32).unwrap();
    bklt.set_high().unwrap();

    // Configuration for M5Stack Core Development Kit V1.0
    // Puts display in landscape mode with the three buttons at the bottom of screen
    let mut m5stack_display = Builder::ili9342c_rgb565(di)
        .with_display_size(320, 240)
        .with_color_order(ColorOrder::Bgr)
        .with_orientation(Orientation::Portrait(false))
        .with_invert_colors(mipidsi::ColorInversion::Inverted)
        .init(&mut delay::Ets, Some(rst))
        .unwrap();

    // Stack size value - 20,000 for 10 lines,  40,000 for 20 lines
    let _lvgl_thread = thread::Builder::new()
        .stack_size(40000)
        .spawn(move || {
            lvgl::init();

            let buffer = DrawBuffer::<{ (HOR_RES * LINES) as usize }>::default();
            let display = Display::register(buffer, HOR_RES, VER_RES, |refresh| {
                m5stack_display.draw_iter(refresh.as_pixels()).unwrap();
            })
            .unwrap();

            // Create screen and widgets
            let mut screen = display.get_scr_act().unwrap();
            let mut screen_style = Style::default();
            screen_style.set_bg_color(Color::from_rgb((0, 0, 255)));
            screen_style.set_radius(0);
            screen.add_style(Part::Main, &mut screen_style);

            let mut time = Label::new().unwrap();
            let mut style_time = Style::default();
            style_time.set_text_color(Color::from_rgb((255, 255, 255))); // white
            style_time.set_text_align(TextAlign::Center);

            // Custom font requires lvgl-sys in Cargo.toml and 'use lvgl_sys' in this file
            style_time.set_text_font(unsafe { Font::new_raw(lvgl_sys::gotham_bold_80) });

            time.add_style(Part::Main, &mut style_time);

            // Time text will be centered in screen
            time.set_align(Align::Center, 0, 0);

            let mut i = 0;

            loop {
                let start = Instant::now();
                if i > 59 {
                    i = 0;
                }

                let val = CString::new(format!("21:{:02}", i)).unwrap();
                time.set_text(&val).unwrap();
                i += 1;

                lvgl::task_handler();

                // Simulate clock - so sleep for one second so time text is incremented in seconds
                delay::FreeRtos::delay_ms(1000);

                lvgl::tick_inc(Instant::now().duration_since(start));
            }
        })
        .unwrap();

    loop {
        // Don't exit application
        delay::FreeRtos::delay_ms(1000);
    }
}
