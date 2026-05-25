#![cfg_attr(not(feature = "tray"), allow(dead_code))]

use crate::errors::{BridgeError, BridgeResult};

const VEX_TRAY_ICON_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../plugins/revit-csharp/assets/vex-ribbon-32.png"
));

pub(crate) fn vex_tray_icon_rgba() -> BridgeResult<(Vec<u8>, u32, u32)> {
    decode_png_rgba(VEX_TRAY_ICON_PNG)
}

fn decode_png_rgba(bytes: &[u8]) -> BridgeResult<(Vec<u8>, u32, u32)> {
    let cursor = std::io::Cursor::new(bytes);
    let mut decoder = png::Decoder::new(cursor);
    decoder.set_transformations(png::Transformations::ALPHA | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .map_err(|error| BridgeError::Config(format!("could not decode tray icon: {error}")))?;
    let mut buffer = vec![
        0;
        reader
            .output_buffer_size()
            .ok_or_else(|| BridgeError::Config(
                "tray icon output buffer is too large".into()
            ))?
    ];
    let info = reader
        .next_frame(&mut buffer)
        .map_err(|error| BridgeError::Config(format!("could not read tray icon: {error}")))?;
    let pixels = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::Grayscale => pixels
            .iter()
            .flat_map(|gray| [*gray, *gray, *gray, 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Indexed => {
            return Err(BridgeError::Config(
                "tray icon palette was not expanded".into(),
            ));
        }
    };
    Ok((rgba, info.width, info.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_vex_icon_decodes_to_rgba() {
        let (rgba, width, height) = vex_tray_icon_rgba().unwrap();
        assert_eq!((width, height), (32, 32));
        assert_eq!(rgba.len(), (width * height * 4) as usize);
    }
}
