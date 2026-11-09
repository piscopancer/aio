//! Bandcamp, asked through the search its own site uses, which needs no key. It holds many songs of small labels and
//! artists who sell their music themselves and are missing from the big stores; the page of each found track adds its
//! length, date, genre and place on the album.

use std::thread;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{RESULTS, ReadJson, agent};
use crate::application::error::AppResult;
use crate::application::ports::{SongLookup, TagFinder};
use crate::application::views::{FoundCover, TagMatch, TagSource};
use crate::domain::tag_search::{genre_of, year_of};
use crate::infrastructure::tools::fetch_error;

const SEARCH: &str = "https://bandcamp.com/api/bcsearch_public_api/1/autocomplete_elastic";

/// The search filter that keeps only tracks, leaving out albums, artists and labels.
const TRACKS: &str = "t";

/// How many of the found tracks get their page read; the rest are listed with what the search tells.
const DETAILED: usize = 5;

/// Where a track page keeps its tags for search engines, as schema.org JSON.
const PAGE_DATA: (&str, &str) = ("<script type=\"application/ld+json\">", "</script>");

#[derive(Serialize)]
struct Query<'a> {
  search_text: &'a str,
  search_filter: &'a str,
  full_page: bool,
  fan_id: Option<u64>,
}

#[derive(Deserialize)]
struct Search {
  auto: Results,
}

#[derive(Deserialize)]
struct Results {
  #[serde(default)]
  results: Vec<Found>,
}

#[derive(Deserialize)]
struct Found {
  #[serde(rename = "type")]
  kind: String,
  name: String,
  band_name: Option<String>,
  album_name: Option<String>,
  art_id: Option<u64>,
  item_url_path: Option<String>,
}

/// The parts of a track page the search leaves out; a page that fails to load leaves them out too.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Details {
  /// Such as `P00H02M19S`.
  duration: Option<String>,
  /// Such as `14 Apr 2022 23:05:04 GMT`.
  date_published: Option<String>,
  /// Bandcamp's own genre first, then the artist's free tags and their town.
  #[serde(default)]
  keywords: Value,
  #[serde(default)]
  additional_property: Vec<Property>,
}

#[derive(Deserialize)]
struct Property {
  name: String,
  value: Value,
}

impl Details {
  fn track_no(&self) -> Option<u32> {
    let property = self
      .additional_property
      .iter()
      .find(|property| property.name == "tracknum")?;
    u32::try_from(property.value.as_u64()?)
      .ok()
      .filter(|&number| number > 0)
  }

  fn genre(&self) -> Option<String> {
    let first = match &self.keywords {
      Value::Array(words) => words.first()?.as_str()?,
      Value::String(words) => words.split(',').next()?,
      _ => return None,
    };
    genre_of(first)
  }
}

/// The length an ISO 8601 duration such as `P00H02M19S` names.
fn duration_ms(text: &str) -> Option<u32> {
  let mut seconds = 0;
  let mut number = String::new();
  for char in text.strip_prefix('P')?.chars() {
    match char {
      '0'..='9' => number.push(char),
      'H' | 'M' | 'S' => {
        let unit = match char {
          'H' => 3600,
          'M' => 60,
          _ => 1,
        };
        seconds += number.parse::<u32>().ok()? * unit;
        number.clear();
      }
      _ => return None,
    }
  }
  Some(seconds * 1000)
}

impl Found {
  fn into_match(self, details: Details) -> TagMatch {
    TagMatch {
      source: TagSource::Bandcamp,
      artists: self.band_name.into_iter().collect(),
      album: self.album_name,
      // the date is written day first, as `14 Apr 2022 23:05:04 GMT`
      year: details
        .date_published
        .as_deref()
        .and_then(|date| date.split_whitespace().nth(2))
        .and_then(year_of),
      track_no: details.track_no(),
      disc_no: None,
      genre: details.genre(),
      duration_ms: details.duration.as_deref().and_then(duration_ms),
      cover: self.art_id.map(|id| FoundCover {
        thumb: format!("https://f4.bcbits.com/img/a{id}_2.jpg"),
        full: format!("https://f4.bcbits.com/img/a{id}_10.jpg"),
      }),
      title: self.name,
    }
  }
}

pub struct Bandcamp {
  http: ureq::Agent,
}

impl Bandcamp {
  pub fn new() -> Self {
    Self { http: agent() }
  }

  fn details(&self, page: &str) -> Details {
    let html = self
      .http
      .get(page)
      .call()
      .ok()
      .and_then(|mut response| response.body_mut().read_to_string().ok());
    html
      .as_deref()
      .and_then(|html| {
        let (open, close) = PAGE_DATA;
        let data = &html[html.find(open)? + open.len()..];
        serde_json::from_str(&data[..data.find(close)?]).ok()
      })
      .unwrap_or_default()
  }
}

impl TagFinder for Bandcamp {
  fn source(&self) -> TagSource {
    TagSource::Bandcamp
  }

  fn find(&self, song: &SongLookup<'_>) -> AppResult<Vec<TagMatch>> {
    let words = song.words();
    let query = Query {
      search_text: &words,
      search_filter: TRACKS,
      full_page: false,
      fan_id: None,
    };
    let search: Search = self
      .http
      .post(SEARCH)
      .header("Content-Type", "application/json")
      .send(serde_json::to_string(&query).map_err(fetch_error)?)
      .map_err(fetch_error)?
      .body_mut()
      .json()?;
    let tracks: Vec<Found> = search
      .auto
      .results
      .into_iter()
      .filter(|found| found.kind == TRACKS)
      .take(RESULTS)
      .collect();
    let details: Vec<Details> = thread::scope(|scope| {
      let pages: Vec<_> = tracks
        .iter()
        .take(DETAILED)
        .map(|track| scope.spawn(|| track.item_url_path.as_deref().map(|page| self.details(page))))
        .collect();
      pages
        .into_iter()
        .map(|page| page.join().ok().flatten().unwrap_or_default())
        .collect()
    });
    let mut details = details.into_iter();
    Ok(
      tracks
        .into_iter()
        .map(|track| track.into_match(details.next().unwrap_or_default()))
        .collect(),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn an_iso_duration_turns_into_milliseconds() {
    assert_eq!(duration_ms("P00H02M19S"), Some(139_000));
    assert_eq!(duration_ms("P1H0M5S"), Some(3_605_000));
    assert_eq!(duration_ms("2:19"), None);
  }

  #[test]
  fn the_first_keyword_is_the_genre() {
    let details: Details = serde_json::from_str(
      r#"{"keywords": ["Hip-Hop/Rap", "lofi", "New York"], "additionalProperty": [{"name": "tracknum", "value": 3}]}"#,
    )
    .unwrap();
    assert_eq!(details.genre().as_deref(), Some("Hip-Hop/Rap"));
    assert_eq!(details.track_no(), Some(3));
  }
}
