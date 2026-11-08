use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, ImageResult};

use crate::application::error::{AppError, AppResult};
use crate::application::ports::CoverImages;

/// Side of the square covers the app writes, in pixels.
pub const COVER_SIDE: u32 = 1000;

/// The image cut to a centred square no larger than `COVER_SIDE` and encoded as JPEG, the cover shape the app shows.
pub fn square_jpeg(image: &DynamicImage) -> ImageResult<Vec<u8>> {
  let side = image.width().min(image.height());
  let square = image.crop_imm((image.width() - side) / 2, (image.height() - side) / 2, side, side);
  let square = if side > COVER_SIDE {
    square.resize_exact(COVER_SIDE, COVER_SIDE, image::imageops::FilterType::Lanczos3)
  } else {
    square
  };
  let mut jpeg = Cursor::new(Vec::new());
  square.to_rgb8().write_to(&mut jpeg, ImageFormat::Jpeg)?;
  Ok(jpeg.into_inner())
}

/// Folder of cover files named by their content hash, so identical artwork is stored once.
pub struct CoverStore {
  dir: PathBuf,
}

impl CoverStore {
  pub fn new(dir: PathBuf) -> Self {
    Self { dir }
  }

  /// Writes the image bytes unless the same bytes are stored already, and returns the file's path.
  pub fn store(&self, data: &[u8], ext: &str) -> io::Result<PathBuf> {
    let hash = blake3::hash(data).to_hex();
    let path = self.dir.join(format!("{}.{ext}", &hash[..32]));
    if !path.exists() {
      fs::create_dir_all(&self.dir)?;
      fs::write(&path, data)?;
    }
    Ok(path)
  }
}

/// How many hue ranges the colour wheel is split into when the dominant colour is chosen.
const HUE_BUCKETS: usize = 24;

/// The colour that covers most of the image: pixels are grouped by hue, weighted by how colourful they are, and the
/// heaviest group's average wins; an image with almost no colour gives its average grey.
pub fn dominant_color(image: &DynamicImage) -> [u8; 3] {
  let small = image.thumbnail(64, 64).to_rgb8();
  let mut buckets = [(0.0_f64, [0.0_f64; 3]); HUE_BUCKETS];
  let mut grey = (0.0_f64, [0.0_f64; 3]);
  for pixel in small.pixels() {
    let rgb = pixel.0.map(|c| f64::from(c) / 255.0);
    let [r, g, b] = rgb;
    let max = r.max(g).max(b);
    let chroma = max - r.min(g).min(b);
    let (weight, target) = if chroma < 0.12 || max < 0.12 {
      (1.0, &mut grey)
    } else {
      let hue = if max == r {
        60.0 * ((g - b) / chroma).rem_euclid(6.0)
      } else if max == g {
        60.0 * ((b - r) / chroma + 2.0)
      } else {
        60.0 * ((r - g) / chroma + 4.0)
      };
      let bucket = (hue / 360.0 * HUE_BUCKETS as f64) as usize % HUE_BUCKETS;
      (chroma, &mut buckets[bucket])
    };
    target.0 += weight;
    for (sum, channel) in target.1.iter_mut().zip(rgb) {
      *sum += weight * channel;
    }
  }
  let colourful: f64 = buckets.iter().map(|bucket| bucket.0).sum();
  let pixels = f64::from(small.width() * small.height());
  let (weight, sums) = match buckets.iter().max_by(|a, b| a.0.total_cmp(&b.0)) {
    // a few coloured specks do not make a grey image colourful
    Some(best) if colourful > pixels * 0.02 => *best,
    _ => grey,
  };
  if weight == 0.0 {
    return [0, 0, 0];
  }
  sums.map(|sum| (sum / weight * 255.0).round() as u8)
}

impl CoverImages for CoverStore {
  fn import(&self, file: &Path) -> AppResult<String> {
    let image = image::open(file).map_err(unusable)?;
    let jpeg = square_jpeg(&image).map_err(unusable)?;
    let path = self.store(&jpeg, "jpg").map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(path.to_string_lossy().into_owned())
  }

  fn dominant_color(&self, file: &Path) -> AppResult<String> {
    let [r, g, b] = dominant_color(&image::open(file).map_err(unusable)?);
    Ok(format!("#{r:02x}{g:02x}{b:02x}"))
  }
}

fn unusable(error: image::ImageError) -> AppError {
  AppError::Image(error.to_string())
}

#[cfg(test)]
mod tests {
  use image::{Rgb, RgbImage};

  use super::*;

  #[test]
  fn the_colour_beats_a_larger_black_background() {
    // an orange star on black, like a typical cover: black covers more, yet orange is the colour
    let image = RgbImage::from_fn(40, 40, |x, _| if x < 12 { Rgb([220, 120, 80]) } else { Rgb([0, 0, 0]) });
    let [r, g, b] = dominant_color(&DynamicImage::ImageRgb8(image));
    assert!(r > 200 && (100..140).contains(&g) && b < 100, "{r} {g} {b}");
  }

  #[test]
  fn a_grey_image_gives_its_grey() {
    let image = RgbImage::from_pixel(10, 10, Rgb([90, 90, 90]));
    assert_eq!(dominant_color(&DynamicImage::ImageRgb8(image)), [90, 90, 90]);
  }
}
