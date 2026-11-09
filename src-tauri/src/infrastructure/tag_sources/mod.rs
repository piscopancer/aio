//! Online music databases that find the tags and covers of a song: AcoustID by its sound, then MusicBrainz, the
//! iTunes stores, Deezer, Bandcamp and YouTube Music by its title and artist. None of them needs an account.

mod acoustid;
mod bandcamp;
mod deezer;
mod itunes;
mod musicbrainz;
mod youtube_music;

use std::time::Duration;

pub use acoustid::AcoustId;
pub use bandcamp::Bandcamp;
pub use deezer::Deezer;
pub use itunes::Itunes;
pub use musicbrainz::MusicBrainz;
pub use youtube_music::YoutubeMusic;

use serde::de::DeserializeOwned;

use super::tools::fetch_error;
use crate::application::error::AppResult;
use crate::application::ports::WebImages;
use crate::application::views::FoundCover;

/// How the app names itself to the databases; MusicBrainz asks for a way to reach the author of a client.
const CLIENT: &str = concat!(
  "Aio/",
  env!("CARGO_PKG_VERSION"),
  " ( https://github.com/piscopancer/aio )"
);

/// How many songs each database is asked for.
const RESULTS: usize = 8;

/// An HTTP client that names the app and gives up on a database that does not answer in time.
fn agent() -> ureq::Agent {
  ureq::Agent::config_builder()
    .user_agent(CLIENT)
    .timeout_global(Some(Duration::from_secs(20)))
    .build()
    .into()
}

/// Reads an answer of a database as the JSON it is.
trait ReadJson {
  fn json<T: DeserializeOwned>(&mut self) -> AppResult<T>;
}

impl ReadJson for ureq::Body {
  fn json<T: DeserializeOwned>(&mut self) -> AppResult<T> {
    let text = self.read_to_string().map_err(fetch_error)?;
    serde_json::from_str(&text).map_err(fetch_error)
  }
}

/// The front cover the Cover Art Archive keeps for a MusicBrainz release group, which the picture may turn out to
/// lack; the dialog then shows none.
fn release_group_cover(id: &str) -> FoundCover {
  let base = format!("https://coverartarchive.org/release-group/{id}");
  FoundCover {
    thumb: format!("{base}/front-250"),
    full: format!("{base}/front-1200"),
  }
}

/// Fetches found cover images.
pub struct CoverDownloads {
  http: ureq::Agent,
}

impl CoverDownloads {
  pub fn new() -> Self {
    Self { http: agent() }
  }
}

impl WebImages for CoverDownloads {
  fn fetch(&self, url: &str) -> AppResult<Vec<u8>> {
    let mut response = self.http.get(url).call().map_err(fetch_error)?;
    response
      .body_mut()
      .with_config()
      .limit(32 * 1024 * 1024)
      .read_to_vec()
      .map_err(fetch_error)
  }
}
