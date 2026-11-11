//! White SVG icons drawn into pixels, for the shell's surfaces that take bitmaps rather than the webview's own icons:
//! the taskbar preview's buttons and the tray menu.

use resvg::{tiny_skia, usvg};

/// How much of each pixel a white SVG icon covers when drawn into a square of `size` pixels, row by row from the top.
pub fn coverage(svg: &[u8], size: u32) -> Option<Vec<u8>> {
  let tree = usvg::Tree::from_data(svg, &usvg::Options::default()).ok()?;
  let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
  let scale = size as f32 / tree.size().width();
  resvg::render(
    &tree,
    tiny_skia::Transform::from_scale(scale, scale),
    &mut pixmap.as_mut(),
  );
  // the icons are white, so the colour is white wherever anything is drawn and only the alpha varies
  Some(pixmap.pixels().iter().map(|pixel| pixel.alpha()).collect())
}

/// Side in pixels of an icon that is `logical` pixels wide at 100%, on a display of `scale`.
pub fn pixels(logical: f64, scale: f64) -> u32 {
  // rounding keeps the icon a whole number of pixels at any display scale
  #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
  let size = (logical * scale).round() as u32;
  size
}
