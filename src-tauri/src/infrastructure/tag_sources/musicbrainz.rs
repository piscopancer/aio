//! MusicBrainz, the open encyclopedia of recordings, searched by title, artist and album.

use serde::Deserialize;

use super::{RESULTS, ReadJson, agent, release_group_cover};
use crate::application::error::AppResult;
use crate::application::ports::{SongLookup, TagFinder};
use crate::application::views::{TagMatch, TagSource};
use crate::domain::tag_search::{genre_of, year_of};
use crate::infrastructure::tools::fetch_error;

const API: &str = "https://musicbrainz.org/ws/2/recording";

#[derive(Deserialize)]
struct Search {
  #[serde(default)]
  recordings: Vec<Recording>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Recording {
  title: String,
  /// Milliseconds.
  length: Option<u32>,
  #[serde(default)]
  artist_credit: Vec<Credit>,
  first_release_date: Option<String>,
  #[serde(default)]
  releases: Vec<Release>,
  #[serde(default)]
  tags: Vec<Tag>,
}

#[derive(Deserialize)]
struct Credit {
  name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Release {
  title: String,
  date: Option<String>,
  release_group: Option<ReleaseGroup>,
  #[serde(default)]
  media: Vec<Medium>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ReleaseGroup {
  id: String,
  primary_type: Option<String>,
}

#[derive(Deserialize)]
struct Medium {
  position: Option<u32>,
  #[serde(default)]
  track: Vec<MediumTrack>,
}

#[derive(Deserialize)]
struct MediumTrack {
  number: Option<String>,
}

#[derive(Deserialize)]
struct Tag {
  name: String,
  #[serde(default)]
  count: i32,
}

impl Recording {
  /// The recording as found tags, on the album it first came out on; a single or a compilation counts only when it
  /// is on no album.
  fn into_match(self, source: TagSource) -> TagMatch {
    let album_first = |release: &&Release| {
      release
        .release_group
        .as_ref()
        .is_some_and(|group| group.primary_type.as_deref() == Some("Album"))
    };
    let release = self.releases.iter().find(album_first).or(self.releases.first());
    let medium = release.and_then(|release| release.media.first());
    let genre = self
      .tags
      .iter()
      .max_by_key(|tag| tag.count)
      .and_then(|tag| genre_of(&tag.name));
    TagMatch {
      source,
      artists: self.artist_credit.into_iter().map(|credit| credit.name).collect(),
      album: release.map(|release| release.title.clone()),
      year: self
        .first_release_date
        .as_deref()
        .or(release.and_then(|release| release.date.as_deref()))
        .and_then(year_of),
      track_no: medium
        .and_then(|medium| medium.track.first())
        .and_then(|track| track.number.as_deref()?.parse().ok()),
      disc_no: medium.and_then(|medium| medium.position),
      genre,
      duration_ms: self.length,
      cover: release
        .and_then(|release| release.release_group.as_ref())
        .map(|group| release_group_cover(&group.id)),
      title: self.title,
    }
  }
}

/// A phrase for a search field of MusicBrainz, its quotes and backslashes escaped.
fn phrase(text: &str) -> String {
  format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

pub struct MusicBrainz {
  http: ureq::Agent,
}

impl MusicBrainz {
  pub fn new() -> Self {
    Self { http: agent() }
  }
}

impl TagFinder for MusicBrainz {
  fn source(&self) -> TagSource {
    TagSource::MusicBrainz
  }

  fn find(&self, song: &SongLookup<'_>) -> AppResult<Vec<TagMatch>> {
    let mut query = format!("recording:{}", phrase(song.title));
    if let Some(artist) = song.artist {
      query.push_str(&format!(" AND artist:{}", phrase(artist)));
    }
    let search: Search = self
      .http
      .get(API)
      .query("query", &query)
      .query("fmt", "json")
      .query("limit", RESULTS.to_string())
      .call()
      .map_err(fetch_error)?
      .body_mut()
      .json()?;
    Ok(
      search
        .recordings
        .into_iter()
        .map(|recording| recording.into_match(TagSource::MusicBrainz))
        .collect(),
    )
  }
}
