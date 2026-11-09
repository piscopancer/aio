//! Deezer, whose search finds songs by title and artist and whose track pages add the release date and the place on
//! the album.

use std::thread;

use serde::Deserialize;

use super::{RESULTS, ReadJson, agent};
use crate::application::error::AppResult;
use crate::application::ports::{SongLookup, TagFinder};
use crate::application::views::{FoundCover, TagMatch, TagSource};
use crate::domain::tag_search::year_of;
use crate::infrastructure::tools::fetch_error;

const API: &str = "https://api.deezer.com";

/// How many of the found songs get their track page read for the date and numbers; Deezer limits how often it may
/// be asked.
const DETAILED: usize = 5;

#[derive(Deserialize)]
struct Search {
  #[serde(default)]
  data: Vec<Song>,
}

#[derive(Deserialize)]
struct Song {
  id: u64,
  title: String,
  /// Seconds.
  duration: Option<u32>,
  artist: Option<Artist>,
  album: Option<Album>,
}

#[derive(Deserialize)]
struct Artist {
  name: String,
}

#[derive(Deserialize)]
struct Album {
  title: String,
  cover_medium: Option<String>,
  cover_xl: Option<String>,
}

/// The parts of a track page the search leaves out; a page that fails to load leaves them out too.
#[derive(Deserialize, Default)]
struct Details {
  release_date: Option<String>,
  track_position: Option<u32>,
  disk_number: Option<u32>,
  #[serde(default)]
  contributors: Vec<Artist>,
}

impl Song {
  fn into_match(self, details: Details) -> TagMatch {
    let artists = if details.contributors.is_empty() {
      self.artist.into_iter().map(|artist| artist.name).collect()
    } else {
      details.contributors.into_iter().map(|artist| artist.name).collect()
    };
    let cover = self.album.as_ref().and_then(|album| {
      Some(FoundCover {
        thumb: album.cover_medium.clone()?,
        full: album.cover_xl.clone()?,
      })
    });
    TagMatch {
      source: TagSource::Deezer,
      title: self.title,
      artists,
      album: self.album.map(|album| album.title),
      year: details.release_date.as_deref().and_then(year_of),
      track_no: details.track_position,
      disc_no: details.disk_number,
      genre: None,
      duration_ms: self.duration.map(|seconds| seconds * 1000),
      cover,
    }
  }
}

pub struct Deezer {
  http: ureq::Agent,
}

impl Deezer {
  pub fn new() -> Self {
    Self { http: agent() }
  }

  fn details(&self, id: u64) -> Details {
    self
      .http
      .get(&format!("{API}/track/{id}"))
      .call()
      .ok()
      .and_then(|mut response| response.body_mut().json().ok())
      .unwrap_or_default()
  }
}

impl TagFinder for Deezer {
  fn source(&self) -> TagSource {
    TagSource::Deezer
  }

  fn find(&self, song: &SongLookup<'_>) -> AppResult<Vec<TagMatch>> {
    // the search by fields (`track:"…" artist:"…"`) finds nothing, so the artist and title are asked as plain words
    let search: Search = self
      .http
      .get(&format!("{API}/search"))
      .query("q", song.words())
      .query("limit", RESULTS.to_string())
      .call()
      .map_err(fetch_error)?
      .body_mut()
      .json()?;
    let songs: Vec<Song> = search.data.into_iter().take(RESULTS).collect();
    let details: Vec<Details> = thread::scope(|scope| {
      let pages: Vec<_> = songs
        .iter()
        .take(DETAILED)
        .map(|song| scope.spawn(|| self.details(song.id)))
        .collect();
      pages.into_iter().map(|page| page.join().unwrap_or_default()).collect()
    });
    let mut details = details.into_iter();
    Ok(
      songs
        .into_iter()
        .map(|song| song.into_match(details.next().unwrap_or_default()))
        .collect(),
    )
  }
}
