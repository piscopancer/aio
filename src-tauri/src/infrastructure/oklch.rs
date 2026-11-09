//! Conversions between sRGB and OKLCH, the perceptual colour space in which lightness and colourfulness can be changed
//! without the hue drifting, after Björn Ottosson's Oklab.

/// A colour as perceived lightness (0 to 1), chroma (0 for grey, around 0.3 at most in sRGB) and hue in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
  pub l: f64,
  pub c: f64,
  pub h: f64,
}

impl Oklch {
  pub fn from_srgb(rgb: [u8; 3]) -> Self {
    let [r, g, b] = rgb.map(|channel| to_linear(f64::from(channel) / 255.0));
    let [l, m, s] = [
      0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b,
      0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b,
      0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b,
    ]
    .map(f64::cbrt);
    let lightness = 0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s;
    let a = 1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s;
    let b = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s;
    Self {
      l: lightness,
      c: a.hypot(b),
      h: b.atan2(a).to_degrees().rem_euclid(360.0),
    }
  }

  /// The colour as Oklab coordinates: lightness and the two opposing colour axes, in which plain distances match how
  /// different two colours look.
  pub fn lab(self) -> [f64; 3] {
    let hue = self.h.to_radians();
    [self.l, self.c * hue.cos(), self.c * hue.sin()]
  }

  pub fn from_lab([l, a, b]: [f64; 3]) -> Self {
    Self {
      l,
      c: a.hypot(b),
      h: b.atan2(a).to_degrees().rem_euclid(360.0),
    }
  }

  /// The colour in sRGB, with its chroma lowered as far as needed to fit, so the lightness and hue stay as asked.
  pub fn to_srgb(self) -> [u8; 3] {
    let mut colour = self;
    loop {
      if let Some(rgb) = colour.linear_rgb() {
        return rgb.map(|channel| (from_linear(channel) * 255.0).round() as u8);
      }
      colour.c *= 0.95;
      if colour.c < 0.001 {
        colour.c = 0.0;
      }
    }
  }

  /// The colour in linear sRGB, or `None` outside its gamut; grey is always inside.
  fn linear_rgb(self) -> Option<[f64; 3]> {
    let (a, b) = (self.c * self.h.to_radians().cos(), self.c * self.h.to_radians().sin());
    let [l, m, s] = [
      self.l + 0.396_337_777_4 * a + 0.215_803_757_3 * b,
      self.l - 0.105_561_345_8 * a - 0.063_854_172_8 * b,
      self.l - 0.089_484_177_5 * a - 1.291_485_548 * b,
    ]
    .map(|channel| channel.powi(3));
    let rgb = [
      4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
      -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
      -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
    ];
    let inside = |channel: f64| (-1e-4..=1.0 + 1e-4).contains(&channel);
    (self.c == 0.0 || rgb.iter().all(|&channel| inside(channel))).then(|| rgb.map(|channel| channel.clamp(0.0, 1.0)))
  }
}

fn to_linear(channel: f64) -> f64 {
  if channel <= 0.040_45 {
    channel / 12.92
  } else {
    ((channel + 0.055) / 1.055).powf(2.4)
  }
}

fn from_linear(channel: f64) -> f64 {
  if channel <= 0.003_130_8 {
    channel * 12.92
  } else {
    1.055 * channel.powf(1.0 / 2.4) - 0.055
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn colours_survive_the_round_trip() {
    for rgb in [
      [255, 255, 255],
      [0, 0, 0],
      [220, 120, 80],
      [30, 60, 200],
      [128, 128, 128],
    ] {
      assert_eq!(Oklch::from_srgb(rgb).to_srgb(), rgb);
    }
  }

  #[test]
  fn a_colour_beyond_srgb_keeps_its_hue() {
    let colour = Oklch {
      l: 0.4,
      c: 0.4,
      h: 150.0,
    };
    let fitted = Oklch::from_srgb(colour.to_srgb());
    assert!(
      (fitted.h - 150.0).abs() < 3.0 && (fitted.l - 0.4).abs() < 0.01,
      "{fitted:?}"
    );
  }
}
