//! Software renderer helper for the wgpu backend.
//!
//! Renders a `MinimalSoftwareWindow` into an RGBA pixel buffer, converting
//! from Slint's premultiplied-alpha output to straight-alpha bytes for
//! uploading to an sRGB wgpu texture.
//!
//! Adapted from truce-slint's platform.rs.

use slint::platform::software_renderer::{MinimalSoftwareWindow, PremultipliedRgbaColor};

/// Un-premultiply LUT: for each alpha byte α, `(255 * 255 / α) * 257`.
/// Maps α → multiplier so `(c * inv_a) >> 16` recovers straight-alpha c.
const UNPREMUL_LUT: [u32; 256] = {
    let mut lut = [0u32; 256];
    let mut i = 1u32;
    while i < 256 {
        lut[i as usize] = u32::MAX / i;
        i += 1;
    }
    lut
};

/// Render a `MinimalSoftwareWindow` into a straight-alpha RGBA byte buffer.
///
/// `rgba_buf` is cleared and filled with `width * height * 4` bytes.
/// `px_buf` is reused as scratch space.
pub fn render_to_rgba(
    window: &MinimalSoftwareWindow,
    width: u32,
    height: u32,
    px_buf: &mut Vec<PremultipliedRgbaColor>,
    rgba_buf: &mut Vec<u8>,
) {
    let pixel_count = (width * height) as usize;
    px_buf.resize(pixel_count, PremultipliedRgbaColor::default());

    window.draw_if_needed(|renderer| {
        renderer.render(px_buf, width as usize);
    });

    rgba_buf.clear();
    rgba_buf.reserve(pixel_count * 4);
    for px in px_buf.iter() {
        let bytes = if px.alpha == 0 {
            [0, 0, 0, 0]
        } else if px.alpha == 255 {
            [px.red, px.green, px.blue, 255]
        } else {
            #[allow(clippy::cast_possible_truncation)]
            let inv_a = UNPREMUL_LUT[px.alpha as usize];
            [
                ((u32::from(px.red) * inv_a) >> 16) as u8,
                ((u32::from(px.green) * inv_a) >> 16) as u8,
                ((u32::from(px.blue) * inv_a) >> 16) as u8,
                px.alpha,
            ]
        };
        rgba_buf.extend_from_slice(&bytes);
    }
}
