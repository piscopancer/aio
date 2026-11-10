//! AcoustID, which recognises a recording by the fingerprint of its sound, so it finds songs whose file names and
//! tags say nothing; it names them as MusicBrainz does.

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rodio::Source;
use rusty_chromaprint::{Configuration, FingerprintCompressor, Fingerprinter};
use serde::Deserialize;

use super::{RESULTS, ReadJson, agent, release_group_cover};
use crate::application::error::{AppError, AppResult};
use crate::application::ports::{SongLookup, TagFinder};
use crate::application::views::{TagMatch, TagSource};
use crate::infrastructure::opus;
use crate::infrastructure::tools::fetch_error;

const API: &str = "https://api.acoustid.org/v2/lookup";

/// The key AcoustID gave this app, sent with every lookup.
const CLIENT_KEY: &str = "XlNoytaFAX";

/// Seconds of the song the fingerprint is taken from, as much as the `fpcalc` tool of AcoustID takes.
const FINGERPRINT_SECONDS: u32 = 120;

/// How sure AcoustID must be that a fingerprint is of a recording for the recording to be offered.
const MIN_SCORE: f64 = 0.5;

#[derive(Deserialize)]
struct Lookup {
  #[serde(default)]
  results: Vec<Found>,
}

#[derive(Deserialize)]
struct Found {
  score: f64,
  #[serde(default)]
  recordings: Vec<Recording>,
}

#[derive(Deserialize)]
struct Recording {
  title: Option<String>,
  /// Seconds.
  duration: Option<f64>,
  #[serde(default)]
  artists: Vec<Artist>,
  #[serde(default)]
  releasegroups: Vec<ReleaseGroup>,
}

#[derive(Deserialize)]
struct Artist {
  name: String,
}

#[derive(Deserialize)]
struct ReleaseGroup {
  id: String,
  title: Option<String>,
  #[serde(rename = "type")]
  kind: Option<String>,
  #[serde(default)]
  releases: Vec<Release>,
}

#[derive(Deserialize)]
struct Release {
  date: Option<Date>,
  #[serde(default)]
  mediums: Vec<Medium>,
}

#[derive(Deserialize)]
struct Date {
  year: Option<u16>,
}

#[derive(Deserialize)]
struct Medium {
  position: Option<u32>,
  #[serde(default)]
  tracks: Vec<MediumTrack>,
}

#[derive(Deserialize)]
struct MediumTrack {
  position: Option<u32>,
}

impl Recording {
  /// The recording as found tags, on the album it is on, or else the first release group it is on, with the year of
  /// its earliest release there.
  fn into_match(self) -> Option<TagMatch> {
    let title = self.title?;
    let group = self
      .releasegroups
      .iter()
      .find(|group| group.kind.as_deref() == Some("Album"))
      .or(self.releasegroups.first());
    let earliest = group.and_then(|group| {
      group
        .releases
        .iter()
        .min_by_key(|release| release.date.as_ref().and_then(|date| date.year).unwrap_or(u16::MAX))
    });
    let medium = earliest.and_then(|release| release.mediums.first());
    Some(TagMatch {
      source: TagSource::AcoustId,
      title,
      artists: self.artists.into_iter().map(|artist| artist.name).collect(),
      album: group.and_then(|group| group.title.clone()),
      year: earliest.and_then(|release| release.date.as_ref()?.year),
      track_no: medium.and_then(|medium| medium.tracks.first()?.position),
      disc_no: medium.and_then(|medium| medium.position),
      genre: None,
      duration_ms: self.duration.map(|seconds| (seconds * 1000.0).round() as u32),
      cover: group.map(|group| release_group_cover(&group.id)),
    })
  }
}

/// The Chromaprint fingerprint of the start of the song at `path`, compressed and encoded as AcoustID takes it.
fn fingerprint(path: &Path) -> AppResult<String> {
  let unreadable = |error: &dyn std::fmt::Display| AppError::Audio(format!("{}: {error}", path.display()));
  let decoder = opus::open(path).map_err(|e| unreadable(&e))?;
  let (rate, channels) = (decoder.sample_rate().get(), decoder.channels().get());
  let config = Configuration::preset_test2();
  let mut printer = Fingerprinter::new(&config);
  printer
    .start(rate, channels.into())
    .map_err(|e| unreadable(&format!("{e:?}")))?;
  let wanted = (rate * FINGERPRINT_SECONDS) as usize * usize::from(channels);
  let samples: Vec<i16> = decoder
    .take(wanted)
    .map(|sample| (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16)
    .collect();
  printer.consume(&samples);
  printer.finish();
  let compressed = FingerprintCompressor::from(&config).compress(printer.fingerprint());
  Ok(URL_SAFE_NO_PAD.encode(compressed))
}

pub struct AcoustId {
  http: ureq::Agent,
}

impl AcoustId {
  pub fn new() -> Self {
    Self { http: agent() }
  }
}

impl TagFinder for AcoustId {
  fn source(&self) -> TagSource {
    TagSource::AcoustId
  }

  fn find(&self, song: &SongLookup<'_>) -> AppResult<Vec<TagMatch>> {
    let fingerprint = fingerprint(song.path)?;
    let seconds = ((song.duration_ms + 500) / 1000).to_string();
    let lookup: Lookup = self
      .http
      .post(API)
      .send_form([
        ("client", CLIENT_KEY),
        ("duration", seconds.as_str()),
        ("fingerprint", fingerprint.as_str()),
        ("meta", "recordings releasegroups releases tracks"),
      ])
      .map_err(fetch_error)?
      .body_mut()
      .json()?;
    Ok(
      lookup
        .results
        .into_iter()
        .filter(|found| found.score >= MIN_SCORE)
        .flat_map(|found| found.recordings)
        .filter_map(Recording::into_match)
        .take(RESULTS)
        .collect(),
    )
  }
}
