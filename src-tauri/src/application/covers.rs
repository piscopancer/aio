use std::path::Path;
use std::sync::Arc;

use super::error::AppResult;
use super::ports::CoverImages;

/// The app's own artwork: images the user picks, and the colour pages tint themselves with.
pub struct CoverService {
  covers: Arc<dyn CoverImages>,
}

impl CoverService {
  pub fn new(covers: Arc<dyn CoverImages>) -> Self {
    Self { covers }
  }

  /// Stores a square copy of a picked image, ready to become a playlist's or a download's cover.
  pub fn import(&self, file: &str) -> AppResult<String> {
    self.covers.import(Path::new(file))
  }

  /// The colour that covers most of the image, as `#rrggbb`.
  pub fn color(&self, file: &str) -> AppResult<String> {
    self.covers.dominant_color(Path::new(file))
  }
}
