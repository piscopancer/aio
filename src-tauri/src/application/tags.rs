use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use specta::Type;

use super::error::{AppError, AppResult};
use super::playback::PlayerHandle;
use super::ports::{AudioFiles, CoverChange, LibraryQueries, Notifier, SongFiles, TrackIndex};
use super::views::{Album, Track, TrackTags};
use crate::domain::artists::Artists;
use crate::domain::ids::TrackId;
use crate::domain::tags::{AlbumMove, SongTags, album_move};

/// The tags the edit dialog sends for one song; blank text and missing numbers clear those tags.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackEdit {
  pub title: String,
  /// The credited artists in order, the main one first.
  pub artists: Vec<String>,
  pub album: Option<String>,
  pub year: Option<u16>,
  pub track_no: Option<u32>,
  pub disc_no: Option<u32>,
  pub genre: Option<String>,
  pub cover: CoverChange,
}

/// Reading and editing the tags of songs, keeping every song of an album on that album's cover and year.
pub struct TagService {
  songs: Arc<dyn SongFiles>,
  files: Arc<dyn AudioFiles>,
  index: Arc<dyn TrackIndex>,
  queries: Arc<dyn LibraryQueries>,
  player: Arc<PlayerHandle>,
  notifier: Arc<dyn Notifier>,
}

/// The files one edit rewrites: the song with all its tags, and the other songs of its album with the album's part.
struct Rewrite {
  tags: SongTags,
  cover: CoverChange,
  siblings: Vec<Track>,
}

impl TagService {
  pub fn new(
    songs: Arc<dyn SongFiles>,
    files: Arc<dyn AudioFiles>,
    index: Arc<dyn TrackIndex>,
    queries: Arc<dyn LibraryQueries>,
    player: Arc<PlayerHandle>,
    notifier: Arc<dyn Notifier>,
  ) -> Self {
    Self {
      songs,
      files,
      index,
      queries,
      player,
      notifier,
    }
  }

  /// The song's tags as its file holds them, with the year of its album, which all its songs share.
  pub fn tags(&self, id: TrackId) -> AppResult<TrackTags> {
    let track = self.track(id)?;
    let tags = self.songs.tags(Path::new(&track.path))?;
    let Some(album_id) = track.album_id else {
      return Ok(tags);
    };
    let album = self.queries.album(album_id)?.album;
    Ok(TrackTags {
      year: album_year(&album).or(tags.year),
      ..tags
    })
  }

  /// Writes the edited tags into the song's file, along with the album's cover and year: taken from the album the
  /// song joins, or written into every song of its album when it stays and they change.
  pub fn edit(&self, id: TrackId, edit: TrackEdit) -> AppResult<()> {
    let track = self.track(id)?;
    let before = self.songs.tags(Path::new(&track.path))?;
    let tags = SongTags::edited(
      &edit.title,
      // no artists clears the tag, and the song is credited to nobody
      (!edit.artists.is_empty())
        .then(|| Artists::new(&edit.artists))
        .transpose()?,
      edit.album.as_deref(),
      edit.year,
    )?
    .numbered(edit.track_no, edit.disc_no)?
    .with_genre(edit.genre.as_deref())
    .with_album_artist(before.album_artist.as_deref());
    let plan = self.plan(&track, tags, edit.cover)?;

    let mut changed = vec![track];
    changed.extend(plan.siblings.iter().cloned());
    let paths: Vec<String> = changed.iter().map(|t| t.path.clone()).collect();
    let ids: Vec<TrackId> = changed.iter().map(|t| t.id).collect();
    let result = self.player.while_released(paths.clone(), || {
      // files written before a failure are indexed all the same, so the library matches the disk
      let written = self.write(&paths[0], &plan);
      let indexed = self.reindex(&paths);
      let fresh = self.queries.tracks_by_ids(&ids).unwrap_or_default();
      (written.and(indexed), fresh)
    });
    self.notifier.library_changed();
    result
  }

  fn plan(&self, track: &Track, tags: SongTags, cover: CoverChange) -> AppResult<Rewrite> {
    // a song on no album shares its cover and year with no other song
    let Some((title, owner)) = tags.album_key() else {
      return Ok(Rewrite {
        tags,
        cover,
        siblings: Vec::new(),
      });
    };
    let albums = self.queries.albums()?;
    let target = albums
      .iter()
      .find(|album| album.title == title && album.artist.as_deref() == owner);
    let plan = match (album_move(track.album_id, target.map(|album| Some(album.id))), target) {
      (AlbumMove::Joins(_), Some(album)) => Rewrite {
        tags: tags.with_year(album_year(album)),
        cover: album_cover(album),
        siblings: Vec::new(),
      },
      (AlbumMove::Stays, Some(album)) => {
        let art_changed = cover != CoverChange::Keep || tags.year() != album_year(album);
        let siblings = if art_changed {
          let tracks = self.queries.album(album.id)?.tracks;
          tracks.into_iter().filter(|other| other.id != track.id).collect()
        } else {
          Vec::new()
        };
        let cover = match cover {
          CoverChange::Keep => album_cover(album),
          cover => cover,
        };
        Rewrite { tags, cover, siblings }
      }
      _ => Rewrite {
        tags,
        cover,
        siblings: Vec::new(),
      },
    };
    Ok(plan)
  }

  fn write(&self, path: &str, plan: &Rewrite) -> AppResult<()> {
    self.songs.write_song(Path::new(path), &plan.tags, &plan.cover)?;
    for sibling in &plan.siblings {
      self
        .songs
        .write_album_art(Path::new(&sibling.path), plan.tags.year(), &plan.cover)?;
    }
    Ok(())
  }

  fn reindex(&self, paths: &[String]) -> AppResult<()> {
    let scanned: Vec<_> = paths
      .iter()
      .filter_map(|path| self.files.file(Path::new(path)))
      .filter_map(|file| self.files.read(&file))
      .collect();
    self.index.retag_tracks(&scanned)
  }

  fn track(&self, id: TrackId) -> AppResult<Track> {
    self
      .queries
      .tracks_by_ids(&[id])?
      .into_iter()
      .next()
      .ok_or(AppError::NotFound)
  }
}

fn album_year(album: &Album) -> Option<u16> {
  album.year.and_then(|year| u16::try_from(year).ok())
}

/// The album's cover as a change that puts it into a file, or takes any other picture out when it has none.
fn album_cover(album: &Album) -> CoverChange {
  album
    .cover
    .clone()
    .map_or(CoverChange::Remove, |path| CoverChange::Set { path })
}
