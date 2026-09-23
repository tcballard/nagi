//! Approved Nagi artwork, embedded so internal pages also work before installation.
use gtk::{glib, prelude::*};
fn texture(pixels: i32) -> gtk::gdk::Texture {
    let bytes: &'static [u8] = match pixels {
        ..=16 => include_bytes!("../assets/icons/hicolor/16x16/apps/nagi.png"),
        17..=22 => include_bytes!("../assets/icons/hicolor/22x22/apps/nagi.png"),
        23..=24 => include_bytes!("../assets/icons/hicolor/24x24/apps/nagi.png"),
        25..=32 => include_bytes!("../assets/icons/hicolor/32x32/apps/nagi.png"),
        33..=48 => include_bytes!("../assets/icons/hicolor/48x48/apps/nagi.png"),
        49..=64 => include_bytes!("../assets/icons/hicolor/64x64/apps/nagi.png"),
        65..=128 => include_bytes!("../assets/icons/hicolor/128x128/apps/nagi.png"),
        129..=256 => include_bytes!("../assets/icons/hicolor/256x256/apps/nagi.png"),
        _ => include_bytes!("../assets/icons/hicolor/512x512/apps/nagi.png"),
    };
    gtk::gdk::Texture::from_bytes(&glib::Bytes::from_static(bytes))
        .expect("bundled Nagi PNG is valid")
}
pub fn image(size: i32) -> gtk::Image {
    let image = gtk::Image::from_paintable(Some(&texture(size)));
    image.set_pixel_size(size);
    image.connect_scale_factor_notify(move |image| {
        image.set_paintable(Some(&texture(size * image.scale_factor())));
    });
    image
}
pub const FAVICON: &str = include_str!("../assets/icons/favicon.html");
