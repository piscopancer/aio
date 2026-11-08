//! Lyrics from LRCLIB, an open database of mostly time-synced lyrics that needs no account.

use std::time::Duration;

use serde::Deserialize;

use super::tools::fetch_error;
use crate::application::error::AppResult;
use crate::application::ports::{LyricsFind, LyricsFinder, LyricsQuery, OnlineLyrics};
use crate::application::views::LyricsOrigin;
use crate::domain::lyrics::same_recording;

const API: &str = "https://lrclib.net/api";
/// LRCLIB asks clients to name themselves in the user agent.
const CLIENT: &str = concat!("Aio/", env!("CARGO_PKG_VERSION"));

/// One LRCLIB entry; either lyrics field can be missing or blank.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
  id: u32,
  /// Seconds.
  duration: Option<f64>,
  #[serde(default)]
  instrumental: bool,
  plain_lyrics: Option<String>,
  synced_lyrics: Option<String>,
}

impl Entry {
  fn length_ms(&self) -> Option<u32> {
    self.duration.map(|seconds| (seconds * 1000.0).round() as u32)
  }

  fn synced(&self) -> Option<&str> {
    self.synced_lyrics.as_deref().filter(|text| !text.trim().is_empty())
  }

  fn useful(&self) -> bool {
    self.instrumental
      || self.synced().is_some()
      || self.plain_lyrics.as_deref().is_some_and(|text| !text.trim().is_empty())
  }

  /// Synced lyrics over plain ones; an entry with neither and no instrumental mark has nothing to offer.
  fn into_lyrics(self, fallback_length_ms: u32) -> Option<OnlineLyrics> {
    let length_ms = self.length_ms().unwrap_or(fallback_length_ms);
    let find = if self.instrumental {
      LyricsFind::Instrumental
    } else {
      let text = self
        .synced_lyrics
        .filter(|text| !text.trim().is_empty())
        .or(self.plain_lyrics.filter(|text| !text.trim().is_empty()))?;
      LyricsFind::Text {
        text,
        origin: LyricsOrigin::Lrclib { id: self.id },
      }
    };
    Some(OnlineLyrics { find, length_ms })
  }
}

pub struct Lrclib {
  http: ureq::Agent,
}

impl Lrclib {
  pub fn new() -> Self {
    let http = ureq::Agent::config_builder()
      .user_agent(CLIENT)
      .timeout_global(Some(Duration::from_secs(15)))
      .build()
      .into();
    Self { http }
  }

  /// The entry LRCLIB files under exactly this title, artist, album and length, if any.
  fn exact(&self, song: &LyricsQuery<'_>, artist: &str, album: &str) -> AppResult<Option<Entry>> {
    let seconds = (song.duration_ms + 500) / 1000;
    let response = self
      .http
      .get(&format!("{API}/get"))
      .query("track_name", song.title)
      .query("artist_name", artist)
      .query("album_name", album)
      .query("duration", seconds.to_string())
      .call();
    match response {
      Ok(response) => {
        let body = response.into_body().read_to_string().map_err(fetch_error)?;
        serde_json::from_str(&body).map(Some).map_err(fetch_error)
      }
      Err(ureq::Error::StatusCode(404)) => Ok(None),
      Err(error) => Err(fetch_error(format!("LRCLIB cannot be reached: {error}"))),
    }
  }

  fn search(&self, song: &LyricsQuery<'_>) -> AppResult<Vec<Entry>> {
    let mut request = self.http.get(&format!("{API}/search")).query("track_name", song.title);
    if let Some(artist) = song.artist {
      request = request.query("artist_name", artist);
    }
    let body = request
      .call()
      .map_err(|error| fetch_error(format!("LRCLIB cannot be reached: {error}")))?
      .into_body()
      .read_to_string()
      .map_err(fetch_error)?;
    serde_json::from_str(&body).map_err(fetch_error)
  }
}

impl LyricsFinder for Lrclib {
  fn find(&self, song: &LyricsQuery<'_>) -> AppResult<Option<OnlineLyrics>> {
    if let (Some(artist), Some(album)) = (song.artist, song.album)
      && let Some(lyrics) = self
        .exact(song, artist, album)?
        .and_then(|entry| entry.into_lyrics(song.duration_ms))
    {
      return Ok(Some(lyrics));
    }
    // the closest recording by length among those timed for it, synced lyrics first
    let best = self
      .search(song)?
      .into_iter()
      .filter(|entry| entry.useful() && entry.length_ms().is_some_and(|ms| same_recording(ms, song.duration_ms)))
      .min_by_key(|entry| {
        (
          entry.synced().is_none() && !entry.instrumental,
          entry.length_ms().map_or(u32::MAX, |ms| ms.abs_diff(song.duration_ms)),
        )
      });
    Ok(best.and_then(|entry| entry.into_lyrics(song.duration_ms)))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  #[ignore = "reaches lrclib.net"]
  fn finds_synced_lyrics_live() {
    let found = Lrclib::new()
      .find(&LyricsQuery {
        title: "Never Gonna Give You Up",
        artist: Some("Rick Astley"),
        album: Some("Whenever You Need Somebody"),
        duration_ms: 213_000,
      })
      .unwrap()
      .expect("LRCLIB has this song");
    match found.find {
      LyricsFind::Text { text, .. } => assert!(text.contains("[0")),
      other => panic!("expected lyrics, got {other:?}"),
    }
  }
}
