use std::borrow::Cow;
use std::path::Path;

fn copy_rgba(width: u32, height: u32, bytes: Vec<u8>) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_image(arboard::ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(bytes),
    })
    .map_err(|e| e.to_string())
}

pub fn copy_png_file(path: &Path) -> Result<(), String> {
    let img = image::open(path).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = img.dimensions();
    copy_rgba(w, h, img.into_raw())
}

pub fn copy_png_bytes(png: &[u8]) -> Result<(), String> {
    let img = image::load_from_memory(png).map_err(|e| e.to_string())?.into_rgba8();
    let (w, h) = img.dimensions();
    copy_rgba(w, h, img.into_raw())
}
