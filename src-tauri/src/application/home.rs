//! The home page: what was played last, scored from the play history, and the songs added last. A song lends the time
//! of its latest play to its album and to every artist it credits, wherever it was played from, while a playlist counts
//! only what was played from it.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::hash::Hash;

use super::error::AppResult;
use super::ports::{LibraryQueries, Scores};
use super::views::{Home, HomeEntry, Track};
use crate::domain::ids::TrackId;

/// How many of each kind the recently played hold at most.
const PER_KIND: usize = 12;

/// How many cards the recently played hold of all kinds together.
const CARDS: usize = 20;

/// How many of the songs added last the home page shows.
const ADDED: u32 = 10;

pub fn home(queries: &dyn LibraryQueries) -> AppResult<Home> {
  Ok(Home {
    recent: recent(queries)?,
    added: added_songs(queries)?,
  })
}

/// The recently played songs in their order, which their cards play from.
pub fn recent_songs<Q: LibraryQueries + ?Sized>(queries: &Q) -> AppResult<Vec<Track>> {
  let mut tracks = played(queries, &queries.scores()?)?;
  tracks.truncate(PER_KIND);
  Ok(tracks)
}

/// The songs added last, the newest first, which their cards play from.
pub fn added_songs<Q: LibraryQueries + ?Sized>(queries: &Q) -> AppResult<Vec<Track>> {
  queries.newest_tracks(ADDED)
}

/// The played songs the library still offers, the one played last first.
fn played<Q: LibraryQueries + ?Sized>(queries: &Q, scores: &Scores) -> AppResult<Vec<Track>> {
  let ids: Vec<TrackId> = scores.tracks.iter().map(|(id, _)| *id).collect();
  queries.tracks_by_ids(&ids)
}

fn recent(queries: &dyn LibraryQueries) -> AppResult<Vec<HomeEntry>> {
  let scores = queries.scores()?;
  let tracks = played(queries, &scores)?;
  let track_scores: HashMap<_, _> = scores.tracks.into_iter().collect();
  let playlist_scores: HashMap<_, _> = scores.playlists.into_iter().collect();
  let mut album_scores = HashMap::new();
  let mut artist_scores = HashMap::new();
  for track in &tracks {
    let played_at = track_scores[&track.id];
    if let Some(album) = track.album_id {
      lend(&mut album_scores, album, played_at);
    }
    for artist in &track.artists {
      lend(&mut artist_scores, artist.id, played_at);
    }
  }
  let mut entries: Vec<(u32, HomeEntry)> = Vec::new();
  entries.extend(top(
    tracks,
    |track| track.id,
    &track_scores,
    |track| HomeEntry::Track { track },
  ));
  entries.extend(top(
    queries.albums()?,
    |album| album.id,
    &album_scores,
    |album| HomeEntry::Album { album },
  ));
  entries.extend(top(
    queries.artists()?,
    |artist| artist.id,
    &artist_scores,
    |artist| HomeEntry::Artist { artist },
  ));
  entries.extend(top(
    queries.playlists()?,
    |playlist| playlist.id,
    &playlist_scores,
    |playlist| HomeEntry::Playlist { playlist },
  ));
  // the sort is stable, so a song stays ahead of its album and its artists when they were played at the same time
  entries.sort_by_key(|(score, _)| Reverse(*score));
  Ok(entries.into_iter().take(CARDS).map(|(_, entry)| entry).collect())
}

/// Lends the time a song was played at to the whole it is part of, such as its album, when it is the later play.
fn lend<K: Hash + Eq>(scores: &mut HashMap<K, u32>, whole: K, played_at: u32) {
  scores
    .entry(whole)
    .and_modify(|score| *score = (*score).max(played_at))
    .or_insert(played_at);
}

/// The `PER_KIND` of `items` that score the highest, the highest first, as cards; those without a score are left out.
fn top<T, K: Hash + Eq>(
  items: Vec<T>,
  key: impl Fn(&T) -> K,
  scores: &HashMap<K, u32>,
  card: impl Fn(T) -> HomeEntry,
) -> Vec<(u32, HomeEntry)> {
  let mut scored: Vec<(u32, T)> = items
    .into_iter()
    .filter_map(|item| Some((*scores.get(&key(&item))?, item)))
    .collect();
  scored.sort_by_key(|(score, _)| Reverse(*score));
  scored.truncate(PER_KIND);
  scored.into_iter().map(|(score, item)| (score, card(item))).collect()
}
