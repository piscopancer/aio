//! The iTunes Search API, asked in one country's store; the stores of different countries spell the same artists in
//! their own scripts.

use serde::Deserialize;

use super::{RESULTS, ReadJson, agent};
use crate::application::error::AppResult;
use crate::application::ports::{SongLookup, TagFinder};
use crate::application::views::{FoundCover, TagMatch, TagSource};
use crate::domain::tag_search::{split_credit, year_of};
use crate::infrastructure::tools::fetch_error;

const API: &str = "https://itunes.apple.com/search";

/// The size an artwork address names; iTunes serves the same picture at any size put in its place.
const ARTWORK_SIZE: &str = "100x100bb";

#[derive(Deserialize)]
struct Search {
  #[serde(default)]
  results: Vec<Song>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Song {
  track_name: Option<String>,
  artist_name: Option<String>,
  collection_name: Option<String>,
  release_date: Option<String>,
  track_number: Option<u32>,
  disc_number: Option<u32>,
  primary_genre_name: Option<String>,
  track_time_millis: Option<u32>,
  artwork_url100: Option<String>,
}

impl Song {
  fn into_match(self, source: TagSource) -> Option<TagMatch> {
    Some(TagMatch {
      source,
      title: self.track_name?,
      artists: self.artist_name.as_deref().map(split_credit).unwrap_or_default(),
      album: self.collection_name,
      year: self.release_date.as_deref().and_then(year_of),
      track_no: self.track_number,
      disc_no: self.disc_number,
      genre: self.primary_genre_name,
      duration_ms: self.track_time_millis,
      cover: self.artwork_url100.map(|url| FoundCover {
        thumb: url.replace(ARTWORK_SIZE, "250x250bb"),
        full: url.replace(ARTWORK_SIZE, "1200x1200bb"),
      }),
    })
  }
}

pub struct Itunes {
  http: ureq::Agent,
  /// The two-letter code of the store's country.
  country: &'static str,
  source: TagSource,
}

impl Itunes {
  pub fn russia() -> Self {
    Self {
      http: agent(),
      country: "ru",
      source: TagSource::ItunesRu,
    }
  }

  pub fn united_states() -> Self {
    Self {
      http: agent(),
      country: "us",
      source: TagSource::ItunesUs,
    }
  }
}

impl TagFinder for Itunes {
  fn source(&self) -> TagSource {
    self.source
  }

  fn find(&self, song: &SongLookup<'_>) -> AppResult<Vec<TagMatch>> {
    let search: Search = self
      .http
      .get(API)
      .query("term", song.words())
      .query("entity", "song")
      .query("country", self.country)
      .query("limit", RESULTS.to_string())
      .call()
      .map_err(fetch_error)?
      .body_mut()
      .json()?;
    Ok(
      search
        .results
        .into_iter()
        .filter_map(|song| song.into_match(self.source))
        .collect(),
    )
  }
}
