use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageFormat, ImageResult};

use crate::application::error::{AppError, AppResult};
use crate::application::ports::CoverImages;
use crate::infrastructure::oklch::Oklch;

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

  /// Stores a square JPEG copy of the image, returning its path.
  fn keep(&self, image: &DynamicImage) -> AppResult<String> {
    let jpeg = square_jpeg(image).map_err(unusable)?;
    let path = self
      .store(&jpeg, "jpg")
      .map_err(|e| AppError::Storage(format!("cannot store a cover in {}: {e}", self.dir.display())))?;
    Ok(path.to_string_lossy().into_owned())
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

/// How many hue ranges the colour wheel is split into when the colours of an image are grouped.
const HUE_BUCKETS: usize = 24;

/// A summed group of pixels: their total weight and the weighted sums of their channels.
type Group = (f64, [f64; 3]);

/// The pixels of a small copy of an image grouped by hue, each weighted by how colourful it is, beside the group of
/// the near-grey and near-black ones.
struct HueGroups {
  buckets: [Group; HUE_BUCKETS],
  grey: Group,
  pixels: f64,
}

impl HueGroups {
  fn of(image: &DynamicImage) -> Self {
    let small = image.thumbnail(64, 64).to_rgb8();
    let mut groups = Self {
      buckets: [(0.0, [0.0; 3]); HUE_BUCKETS],
      grey: (0.0, [0.0; 3]),
      pixels: f64::from(small.width() * small.height()),
    };
    for pixel in small.pixels() {
      let rgb = pixel.0.map(|c| f64::from(c) / 255.0);
      let [r, g, b] = rgb;
      let max = r.max(g).max(b);
      let chroma = max - r.min(g).min(b);
      let (weight, target) = if chroma < 0.12 || max < 0.12 {
        (1.0, &mut groups.grey)
      } else {
        let hue = if max == r {
          60.0 * ((g - b) / chroma).rem_euclid(6.0)
        } else if max == g {
          60.0 * ((b - r) / chroma + 2.0)
        } else {
          60.0 * ((r - g) / chroma + 4.0)
        };
        let bucket = (hue / 360.0 * HUE_BUCKETS as f64) as usize % HUE_BUCKETS;
        (chroma, &mut groups.buckets[bucket])
      };
      target.0 += weight;
      for (sum, channel) in target.1.iter_mut().zip(rgb) {
        *sum += weight * channel;
      }
    }
    groups
  }

  /// Whether the image has colour at all: a few coloured specks do not make a grey image colourful.
  fn colourful(&self) -> bool {
    self.buckets.iter().map(|bucket| bucket.0).sum::<f64>() > self.pixels * 0.02
  }

  /// Indices of the hue buckets from the heaviest down.
  fn by_weight(&self) -> Vec<usize> {
    let mut order: Vec<usize> = (0..HUE_BUCKETS).collect();
    order.sort_by(|&a, &b| self.buckets[b].0.total_cmp(&self.buckets[a].0));
    order
  }
}

/// The average colour of a group, black for an empty one.
fn average((weight, sums): Group) -> [u8; 3] {
  if weight == 0.0 {
    return [0, 0, 0];
  }
  sums.map(|sum| (sum / weight * 255.0).round() as u8)
}

/// The colour that covers most of the image: pixels are grouped by hue, weighted by how colourful they are, and the
/// heaviest group's average wins; an image with almost no colour gives its average grey.
pub fn dominant_color(image: &DynamicImage) -> [u8; 3] {
  let groups = HueGroups::of(image);
  match groups.by_weight().first() {
    Some(&best) if groups.colourful() => average(groups.buckets[best]),
    _ => average(groups.grey),
  }
}

/// How many colours a palette has, as many as the lyrics background blends.
pub const PALETTE_SIZE: usize = 4;

/// Lightness the palette colours are held in: dark enough that white text reads on any of them, light enough that
/// their colour still shows.
const PALETTE_LIGHTNESS: (f64, f64) = (0.27, 0.43);

/// How much darker each further shade of a colour is made, so that a cover with few colours still gives the blend
/// some depth without bringing in a hue it does not have.
const SHADE_DARKENING: f64 = 0.07;

/// Chroma the colours of a colourful cover are held in: they are made a little richer, but never garish.
const PALETTE_CHROMA: (f64, f64) = (0.06, 0.15);

/// The most chroma a grey cover's shades keep, enough for a hint of its tint.
const GREY_CHROMA: f64 = 0.02;

/// Side of the small copy of a cover its colours are read from, large enough that a thin stripe of colour survives.
const SAMPLE_SIDE: u32 = 96;

/// How many colour clusters a cover is split into before alike ones are merged.
const CLUSTERS: usize = 8;

/// How many times the clusters are refined; they settle well before that on a cover.
const CLUSTER_ROUNDS: usize = 12;

/// The smallest share of a cover a colour needs to count, so that stray specks stay out while a thin accent stays in.
const MIN_COLOUR_SHARE: f64 = 0.01;

/// Chroma below which a colour counts as black, white or grey and adds no hue to the palette.
const NEUTRAL_CHROMA: f64 = 0.04;

/// Hues closer than this, in degrees, give one palette colour, since the palette's band of lightness and chroma
/// would make them look alike.
const SAME_HUE: f64 = 20.0;

/// How much the harmony of a set of colours counts against their presence on the cover when a palette is chosen.
const HARMONY_WEIGHT: f64 = 0.6;

/// Hue distances, in degrees, at which two colours sit well together, each with how far either way the ease lasts:
/// neighbours, a triad and complements. Distances between them, such as a quarter turn, tend to clash.
const HARMONIOUS_TURNS: [(f64, f64); 3] = [(0.0, 45.0), (120.0, 20.0), (180.0, 35.0)];

/// A colour of the cover with the share of the cover it takes.
#[derive(Debug, Clone, Copy)]
struct CoverColour {
  colour: Oklch,
  share: f64,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
  a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
}

fn hue_distance(a: f64, b: f64) -> f64 {
  let turn = (a - b).rem_euclid(360.0);
  turn.min(360.0 - turn)
}

/// Splits colours into `count` clusters of alike ones, returning each cluster's centre and size. The first centre is
/// the mean colour and each further one the colour farthest from those chosen, so the same cover always gives the
/// same clusters and a small but distinct colour gets one of its own.
fn clusters(points: &[[f64; 3]], count: usize) -> Vec<([f64; 3], usize)> {
  let mean = |members: &mut dyn Iterator<Item = &[f64; 3]>| {
    let (sum, size) = members.fold(([0.0; 3], 0usize), |(sum, size), point| {
      ([sum[0] + point[0], sum[1] + point[1], sum[2] + point[2]], size + 1)
    });
    (size > 0).then(|| sum.map(|total| total / size as f64))
  };
  let Some(first) = mean(&mut points.iter()) else {
    return Vec::new();
  };
  let mut centres = vec![first];
  while centres.len() < count {
    let farthest = points.iter().copied().max_by(|a, b| {
      let near = |point: &[f64; 3]| {
        centres
          .iter()
          .map(|centre| distance(*point, *centre))
          .fold(f64::MAX, f64::min)
      };
      near(a).total_cmp(&near(b))
    });
    match farthest {
      Some(point) if !centres.contains(&point) => centres.push(point),
      _ => break,
    }
  }
  let nearest = |point: [f64; 3], centres: &[[f64; 3]]| {
    (0..centres.len())
      .min_by(|&a, &b| distance(point, centres[a]).total_cmp(&distance(point, centres[b])))
      .unwrap_or(0)
  };
  let mut owners = vec![0; points.len()];
  for _ in 0..CLUSTER_ROUNDS {
    for (owner, &point) in owners.iter_mut().zip(points) {
      *owner = nearest(point, &centres);
    }
    for (index, centre) in centres.iter_mut().enumerate() {
      let mut members = points
        .iter()
        .zip(&owners)
        .filter(|(_, owner)| **owner == index)
        .map(|(point, _)| point);
      if let Some(moved) = mean(&mut members) {
        *centre = moved;
      }
    }
  }
  centres
    .iter()
    .enumerate()
    .map(|(index, &centre)| (centre, owners.iter().filter(|&&owner| owner == index).count()))
    .collect()
}

/// The colours of the image, largest first: its pixels are clustered by how alike they look, black, white and greys are
/// set aside, and clusters of one hue are merged. An image with no colour gives its average, tint included.
fn cover_colours(image: &DynamicImage) -> Vec<CoverColour> {
  let small = image.thumbnail(SAMPLE_SIDE, SAMPLE_SIDE).to_rgb8();
  let points: Vec<[f64; 3]> = small.pixels().map(|pixel| Oklch::from_srgb(pixel.0).lab()).collect();
  let total = points.len().max(1) as f64;
  let mut found = clusters(&points, CLUSTERS);
  found.sort_by_key(|entry| std::cmp::Reverse(entry.1));
  let mut colours: Vec<CoverColour> = Vec::new();
  for (centre, size) in found {
    let share = size as f64 / total;
    let colour = Oklch::from_lab(centre);
    if share < MIN_COLOUR_SHARE || colour.c < NEUTRAL_CHROMA {
      continue;
    }
    match colours
      .iter_mut()
      .find(|known| hue_distance(known.colour.h, colour.h) < SAME_HUE)
    {
      Some(known) => known.share += share,
      None => colours.push(CoverColour { colour, share }),
    }
  }
  if colours.is_empty() {
    let average = points.iter().fold([0.0; 3], |sum, point| {
      [sum[0] + point[0], sum[1] + point[1], sum[2] + point[2]]
    });
    return vec![CoverColour {
      colour: Oklch::from_lab(average.map(|channel| channel / total)),
      share: 1.0,
    }];
  }
  colours
}

/// How well two hues sit together, from 0 for a clash to 1 for an exact neighbour, triad or complement.
fn hue_harmony(a: f64, b: f64) -> f64 {
  let distance = hue_distance(a, b);
  HARMONIOUS_TURNS
    .iter()
    .map(|&(turn, ease)| (1.0 - (distance - turn).abs() / ease).max(0.0))
    .fold(0.0, f64::max)
}

/// The most pleasing set of up to `PALETTE_SIZE` cover colours, largest first: every set is weighed by how much of the
/// cover its colours take and by how well their hues sit together, so a clashing colour loses to a calmer one nearly as
/// large.
fn choose_colours(colours: &[CoverColour]) -> Vec<CoverColour> {
  if colours.len() <= PALETTE_SIZE {
    return colours.to_vec();
  }
  let score = |set: &[usize]| {
    // the root lets a small accent count for more than its area, as it does to the eye
    let presence: f64 = set.iter().map(|&index| colours[index].share.sqrt()).sum();
    let mut harmony = 0.0;
    let mut pairs = 0.0;
    for (position, &a) in set.iter().enumerate() {
      for &b in &set[position + 1..] {
        harmony += hue_harmony(colours[a].colour.h, colours[b].colour.h);
        pairs += 1.0;
      }
    }
    presence + HARMONY_WEIGHT * harmony / pairs
  };
  let mut set: Vec<usize> = (0..PALETTE_SIZE).collect();
  let mut best = set.clone();
  let mut best_score = score(&best);
  // every set of PALETTE_SIZE colours in rising order: at most 70 sets for 4 of 8
  while let Some(place) = (0..PALETTE_SIZE)
    .rev()
    .find(|&place| set[place] < colours.len() - PALETTE_SIZE + place)
  {
    set[place] += 1;
    for next in place + 1..PALETTE_SIZE {
      set[next] = set[next - 1] + 1;
    }
    let candidate = score(&set);
    if candidate > best_score {
      best_score = candidate;
      best.clone_from(&set);
    }
  }
  best.into_iter().map(|index| colours[index]).collect()
}

/// A palette of `PALETTE_SIZE` colours made only from the hues of a cover, strongest first: each is moved into the
/// same band of lightness and chroma, and when the cover has fewer colours, the rest are darker shades of them, so no
/// hue the cover lacks gets in. A grey cover gives shades of its grey.
fn harmonize(base: &[Oklch]) -> Vec<[u8; 3]> {
  if base.is_empty() {
    return vec![[0, 0, 0]; PALETTE_SIZE];
  }
  let grey = base.len() == 1 && base[0].c < PALETTE_CHROMA.0;
  let (low, high) = PALETTE_LIGHTNESS;
  (0..PALETTE_SIZE)
    .map(|index| {
      let source = base[index % base.len()];
      // the how-many-th shade of its source this colour is, each a step darker
      let shade = (index / base.len()) as f64;
      let lightness = (low + (high - low) * source.l - SHADE_DARKENING * shade).max(low - 2.0 * SHADE_DARKENING);
      let chroma = if grey {
        source.c.min(GREY_CHROMA)
      } else {
        (source.c * 1.25).clamp(PALETTE_CHROMA.0, PALETTE_CHROMA.1)
      };
      Oklch {
        l: lightness,
        c: chroma,
        h: source.h,
      }
      .to_srgb()
    })
    .collect()
}

impl CoverImages for CoverStore {
  fn import(&self, file: &Path) -> AppResult<String> {
    self.keep(&image::open(file).map_err(unusable)?)
  }

  fn import_bytes(&self, data: &[u8]) -> AppResult<String> {
    self.keep(&without_letterbox(&image::load_from_memory(data).map_err(unusable)?))
  }

  fn dominant_color(&self, file: &Path) -> AppResult<String> {
    let [r, g, b] = dominant_color(&image::open(file).map_err(unusable)?);
    Ok(hex([r, g, b]))
  }

  fn palette(&self, file: &Path) -> AppResult<Vec<String>> {
    let colours = choose_colours(&cover_colours(&image::open(file).map_err(unusable)?));
    let base: Vec<Oklch> = colours.iter().map(|colour| colour.colour).collect();
    Ok(harmonize(&base).into_iter().map(hex).collect())
  }
}

/// The brightest channel a row may reach and still count as a black bar.
const BAR_BRIGHTNESS: u8 = 24;

/// The image without the black bars above and below it, as a video's picture has around a wide frame; an image
/// that is all dark is kept whole.
fn without_letterbox(image: &DynamicImage) -> DynamicImage {
  let rgb = image.to_rgb8();
  let (width, height) = rgb.dimensions();
  let bar = |y: u32| {
    (0..width).all(|x| {
      rgb
        .get_pixel(x, y)
        .0
        .into_iter()
        .all(|channel| channel <= BAR_BRIGHTNESS)
    })
  };
  let Some(top) = (0..height).find(|&y| !bar(y)) else {
    return image.clone();
  };
  let bottom = (0..height).rev().find(|&y| !bar(y)).unwrap_or(top);
  image.crop_imm(0, top, width, bottom - top + 1)
}

fn hex([r, g, b]: [u8; 3]) -> String {
  format!("#{r:02x}{g:02x}{b:02x}")
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

  fn oklch_of(colours: &[CoverColour]) -> Vec<Oklch> {
    colours.iter().map(|colour| colour.colour).collect()
  }

  #[test]
  fn a_two_coloured_cover_gives_only_its_own_hues() {
    // large, as covers are, so the few blended pixels along the border shrink away when the image is sampled
    let image = RgbImage::from_fn(400, 400, |x, _| {
      if x < 250 {
        Rgb([30, 50, 200])
      } else {
        Rgb([230, 120, 30])
      }
    });
    let colours = oklch_of(&cover_colours(&DynamicImage::ImageRgb8(image)));
    assert_eq!(colours.len(), 2, "{colours:?}");
    let palette: Vec<Oklch> = harmonize(&colours).into_iter().map(Oklch::from_srgb).collect();
    assert_eq!(palette.len(), PALETTE_SIZE);
    for colour in &palette {
      assert!(colour.l < PALETTE_LIGHTNESS.1 + 0.01 && colour.c > 0.04, "{colour:?}");
      assert!(
        colours.iter().any(|source| hue_distance(colour.h, source.h) < 4.0),
        "{colour:?} not of {colours:?}"
      );
    }
  }

  #[test]
  fn a_thin_accent_on_paper_joins_the_palette() {
    // beige paper with a black disc and a thin orange-red stripe, like a constructivist poster
    let image = RgbImage::from_fn(200, 200, |x, y| {
      let (dx, dy) = (f64::from(x) - 120.0, f64::from(y) - 60.0);
      if dx.hypot(dy) < 35.0 {
        Rgb([30, 25, 25])
      } else if (x + y).abs_diff(200) < 4 {
        Rgb([225, 80, 45])
      } else {
        Rgb([205, 171, 133])
      }
    });
    let colours = oklch_of(&cover_colours(&DynamicImage::ImageRgb8(image)));
    assert_eq!(colours.len(), 2, "{colours:?}");
    let beige = Oklch::from_srgb([205, 171, 133]).h;
    let red = Oklch::from_srgb([225, 80, 45]).h;
    assert!(hue_distance(colours[0].h, beige) < 10.0, "{colours:?}");
    assert!(hue_distance(colours[1].h, red) < 10.0, "{colours:?}");
  }

  #[test]
  fn a_grey_cover_gives_shades_of_grey() {
    let image = RgbImage::from_pixel(10, 10, Rgb([200, 200, 200]));
    let palette = harmonize(&oklch_of(&cover_colours(&DynamicImage::ImageRgb8(image))));
    assert_eq!(palette.len(), PALETTE_SIZE);
    assert!(palette.iter().all(|&rgb| Oklch::from_srgb(rgb).c < 0.03), "{palette:?}");
  }

  #[test]
  fn a_clashing_colour_loses_to_a_calmer_one() {
    let colour = |rgb: [u8; 3], share: f64| CoverColour {
      colour: Oklch::from_srgb(rgb),
      share,
    };
    // red, orange, a little blue and yellow, and a slightly larger lime that clashes with red and orange
    let lime = Oklch::from_srgb([150, 200, 40]);
    let colours = [
      colour([200, 40, 40], 0.3),
      colour([230, 130, 40], 0.25),
      colour([40, 60, 200], 0.12),
      colour([150, 200, 40], 0.13),
      colour([230, 200, 40], 0.12),
    ];
    let chosen = oklch_of(&choose_colours(&colours));
    assert_eq!(chosen.len(), PALETTE_SIZE);
    assert!(!chosen.contains(&lime), "{chosen:?}");
  }

  #[test]
  fn hues_a_quarter_turn_apart_clash() {
    assert!(hue_harmony(30.0, 40.0) > 0.7);
    assert!(hue_harmony(30.0, 210.0) > 0.9);
    assert_eq!(hue_harmony(30.0, 115.0), 0.0);
  }

  /// Prints the palette of the cover file named by `AIO_COVER`, to judge it by eye.
  #[test]
  #[ignore]
  fn print_cover_palette() {
    let path = std::env::var("AIO_COVER").expect("AIO_COVER names a cover file");
    let colours = cover_colours(&image::open(path).unwrap());
    for colour in &colours {
      println!(
        "candidate {} {:.3} {:?}",
        hex(colour.colour.to_srgb()),
        colour.share,
        colour.colour
      );
    }
    for colour in harmonize(&oklch_of(&choose_colours(&colours))) {
      println!("palette {} {:?}", hex(colour), Oklch::from_srgb(colour));
    }
  }

  #[test]
  fn black_bars_above_and_below_a_picture_go() {
    let image = image::RgbImage::from_fn(48, 36, |_, y| {
      if (6..30).contains(&y) {
        image::Rgb([200, 120, 40])
      } else {
        image::Rgb([5, 5, 5])
      }
    });
    let trimmed = without_letterbox(&DynamicImage::ImageRgb8(image));
    assert_eq!((trimmed.width(), trimmed.height()), (48, 24));
    let dark = DynamicImage::ImageRgb8(image::RgbImage::new(10, 10));
    assert_eq!(without_letterbox(&dark).height(), 10);
  }
}
