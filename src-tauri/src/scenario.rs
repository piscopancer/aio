//! Development fixtures behind `--scenario`: a made-up library seeded through the same ports the real app uses.

use rand::rngs::StdRng;
use rand::seq::IndexedRandom;
use rand::{Rng, SeedableRng};

use crate::application::error::AppResult;
use crate::application::playback::{PlayerCommand, PlayerHandle};
use crate::application::playlists::PlaylistService;
use crate::application::ports::{FolderRepository, LibraryQueries, ScannedTrack, TrackIndex};
use crate::domain::ids::TrackId;
use crate::domain::library::{FolderPath, LibraryFolders};
use crate::domain::playback::{PlaybackContext, RepeatMode};
use crate::infrastructure::audio::FIXTURE_PREFIX;
use crate::interface::cli::Scenario;

const ARTIST_COUNT: usize = 40;
const SEED: u64 = 0x00a1_c075;

const FIRST: &[&str] = &[
  "Velvet", "Paper", "Golden", "Hollow", "Quiet", "Neon", "Amber", "Silver", "Wild", "Midnight", "Glass", "Northern",
  "Lunar", "Copper", "Soft", "Crimson", "Electric", "Distant", "Burning", "Little",
];
const SECOND: &[&str] = &[
  "Orchards",
  "Harbor",
  "Tigers",
  "Lanterns",
  "Rivers",
  "Engines",
  "Choir",
  "Satellites",
  "Foxes",
  "Gardens",
  "Signals",
  "Owls",
  "Static",
  "Parade",
  "Machines",
  "Pilots",
  "Comets",
  "Ghosts",
  "Apricots",
  "Waves",
];
const WORDS: &[&str] = &[
  "Summer", "Light", "Echo", "Home", "Fire", "Rain", "Dream", "Heart", "Road", "Night", "Ocean", "Sky", "Memory",
  "Gold", "Shadow", "Dust", "Morning", "Window", "Silence", "Stars", "Paper", "Glass", "Wire", "Tide", "Garden",
];
const PLAYLISTS: &[&str] = &[
  "Rainy Day",
  "Late Night Drive",
  "Apricot Sunset",
  "Focus",
  "Morning Run",
];

fn pick<'a>(rng: &mut StdRng, words: &[&'a str]) -> &'a str {
  words.choose(rng).copied().unwrap_or_default()
}

fn phrase(rng: &mut StdRng, max_words: usize) -> String {
  let count = rng.random_range(1..=max_words);
  (0..count).map(|_| pick(rng, WORDS)).collect::<Vec<_>>().join(" ")
}

fn scenario_folder() -> FolderPath {
  let root = if cfg!(windows) { "C:\\" } else { "/" };
  FolderPath::new(format!("{root}Music (scenario)")).expect("the scenario folder path is absolute")
}

/// Fills the empty in-memory store for the chosen scenario.
pub fn seed(
  scenario: Scenario,
  folders: &dyn FolderRepository,
  index: &dyn TrackIndex,
  playlists: &PlaylistService,
  queries: &dyn LibraryQueries,
) -> AppResult<()> {
  if scenario == Scenario::Empty {
    return Ok(());
  }
  let mut rng = StdRng::seed_from_u64(SEED);
  let folder = scenario_folder();
  let mut library = LibraryFolders::default();
  library.add(folder.clone())?;
  folders.save_folders(&library)?;

  let mut tracks = Vec::new();
  let mut artists: Vec<String> = Vec::new();
  while artists.len() < ARTIST_COUNT {
    let artist = format!("{} {}", pick(&mut rng, FIRST), pick(&mut rng, SECOND));
    if artists.contains(&artist) {
      continue;
    }
    for _ in 0..rng.random_range(2..=4) {
      let album = phrase(&mut rng, 3);
      let year = rng.random_range(1995..=2026);
      for number in 1..=rng.random_range(8..=14) {
        tracks.push(ScannedTrack {
          path: format!("{FIXTURE_PREFIX}{}", tracks.len()),
          title: phrase(&mut rng, 4),
          artist: artist.clone(),
          album: album.clone(),
          album_artist: artist.clone(),
          year: Some(year),
          track_no: Some(number),
          disc_no: Some(1),
          duration_ms: rng.random_range(95_000..=330_000),
          modified: 0,
          cover: None,
        });
      }
    }
    artists.push(artist);
  }
  index.index_tracks(&folder, &tracks)?;

  let ids: Vec<TrackId> = queries.tracks()?.into_iter().map(|track| track.id).collect();
  for name in PLAYLISTS {
    let playlist = playlists.create(name, None)?;
    let size = rng.random_range(12..=60);
    let picked: Vec<TrackId> = ids.choose_multiple(&mut rng, size).copied().collect();
    playlists.add_tracks(playlist, &picked)?;
  }
  Ok(())
}

/// Starts playback for scenarios that open mid-song.
pub fn start(scenario: Scenario, player: &PlayerHandle, queries: &dyn LibraryQueries) -> AppResult<()> {
  if scenario != Scenario::Playing {
    return Ok(());
  }
  let albums = queries.albums()?;
  let Some(album) = albums.get(albums.len() / 2) else {
    return Ok(());
  };
  let tracks = queries.album(album.id)?.tracks;
  let Some(track) = tracks.get(tracks.len() / 3) else {
    return Ok(());
  };
  player.send(PlayerCommand::SetShuffle(false));
  player.play(queries, PlaybackContext::Album { id: album.id }, Some(track.id))?;
  player.send(PlayerCommand::SetShuffle(true));
  player.send(PlayerCommand::SetRepeat(RepeatMode::All));
  player.send(PlayerCommand::Seek(track.duration_ms / 2));
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use super::*;
  use crate::infrastructure::sqlite::SqliteStore;

  /// Runs every library read against the seeded library, so a malformed query fails here and not in the app.
  #[test]
  fn every_library_query_runs_on_the_seeded_library() {
    let store = Arc::new(SqliteStore::in_memory().unwrap());
    let playlists = PlaylistService::new(store.clone());
    seed(
      Scenario::Library,
      store.as_ref(),
      store.as_ref(),
      &playlists,
      store.as_ref(),
    )
    .unwrap();
    let queries: &dyn LibraryQueries = store.as_ref();

    let tracks = queries.tracks().unwrap();
    assert!(!tracks.is_empty());
    assert_eq!(queries.tracks_by_ids(&[tracks[0].id]).unwrap().len(), 1);
    queries.folders().unwrap();
    for album in queries.albums().unwrap().iter().take(3) {
      assert!(!queries.album(album.id).unwrap().tracks.is_empty());
    }
    for artist in queries.artists().unwrap().iter().take(3) {
      assert!(!queries.artist(artist.id).unwrap().tracks.is_empty());
    }
    let lists = queries.playlists().unwrap();
    assert!(!lists.is_empty());
    for playlist in &lists {
      assert_eq!(
        queries.playlist(playlist.id).unwrap().tracks.len() as u32,
        playlist.track_count
      );
    }
  }
}
