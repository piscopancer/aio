use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use specta::Type;

use super::error::{AppError, AppResult, Missing};
use super::playback::PlayerHandle;
use super::ports::{AlbumShare, AudioFiles, CoverChange, LibraryQueries, LyricsStore, Notifier, SongFiles, TrackIndex};
use super::views::{Album, Track, TrackTags};
use crate::domain::artists::Artists;
use crate::domain::ids::TrackId;
use crate::domain::tags::{AlbumMove, ReleaseType, SongTags, album_move, own_album_artists};

/// The tags the edit dialog sends for one song; blank text, empty lists and missing numbers clear those tags.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackEdit {
  pub title: String,
  /// The credited artists in order, the main one first.
  pub artists: Vec<String>,
  /// Whose the album is when that is not the main artist alone, as on compilations and joint albums; it goes to
  /// every song of the album.
  pub album_artists: Vec<String>,
  pub album: Option<String>,
  pub year: Option<u16>,
  pub release_type: Option<ReleaseType>,
  pub track_no: Option<u32>,
  pub disc_no: Option<u32>,
  pub genres: Vec<String>,
  pub composers: Vec<String>,
  pub lyricists: Vec<String>,
  pub comment: Option<String>,
  /// Blank clears them, and the song's lyrics are then looked up online like those of any song without.
  pub lyrics: Option<String>,
  pub cover: CoverChange,
}

/// Reading and editing the tags of songs, keeping every song of an album on that album's artists, cover, year and
/// release type.
pub struct TagService {
  songs: Arc<dyn SongFiles>,
  files: Arc<dyn AudioFiles>,
  index: Arc<dyn TrackIndex>,
  queries: Arc<dyn LibraryQueries>,
  lyrics: Arc<dyn LyricsStore>,
  player: Arc<PlayerHandle>,
  notifier: Arc<dyn Notifier>,
}

/// The files one edit rewrites: the song with all its tags, and the other songs of its album with the album's part.
struct Rewrite {
  tags: SongTags,
  cover: CoverChange,
  /// Whether the siblings take the song's album artists too, as when the album changes hands.
  rehomed: bool,
  siblings: Vec<Track>,
}

impl TagService {
  pub fn new(
    songs: Arc<dyn SongFiles>,
    files: Arc<dyn AudioFiles>,
    index: Arc<dyn TrackIndex>,
    queries: Arc<dyn LibraryQueries>,
    lyrics: Arc<dyn LyricsStore>,
    player: Arc<PlayerHandle>,
    notifier: Arc<dyn Notifier>,
  ) -> Self {
    Self {
      songs,
      files,
      index,
      queries,
      lyrics,
      player,
      notifier,
    }
  }

  /// The song's tags as its file holds them, with the year and release type of its album, which all its songs share.
  pub fn tags(&self, id: TrackId) -> AppResult<TrackTags> {
    let track = self.track(id)?;
    let tags = self.songs.tags(Path::new(&track.path))?;
    let Some(album_id) = track.album_id else {
      return Ok(tags);
    };
    let album = self.queries.album(album_id)?.album;
    Ok(TrackTags {
      year: album_year(&album).or(tags.year),
      release_type: album.release_type.or(tags.release_type),
      ..tags
    })
  }

  /// Writes the edited tags into the song's file, along with the album's artists, cover, year and release type:
  /// taken from the album the song joins, or written into every song of its album when it stays and they change.
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
    .with_genres(&edit.genres)
    .with_writers(&edit.composers, &edit.lyricists)
    .with_comment(edit.comment.as_deref())
    .with_lyrics(edit.lyrics.as_deref())
    .with_album_artists(Artists::new(&edit.album_artists).ok())
    .with_release_type(edit.release_type);
    let owners_before = Artists::new(&before.album_artists).ok();
    let album_artists_edited =
      tags.album_artists() != own_album_artists(owners_before.as_ref(), before.artists.first().map(String::as_str));
    let lyrics_edited = tags.lyrics() != before.lyrics.as_deref().map(str::trim).filter(|text| !text.is_empty());
    let plan = self.plan(&track, tags, edit.cover, album_artists_edited)?;

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
    if lyrics_edited {
      // whatever was found online before no longer stands, nor may it be written over the new words
      self.lyrics.forget_lyrics(id)?;
    }
    self.notifier.library_changed();
    result
  }

  fn plan(&self, track: &Track, tags: SongTags, cover: CoverChange, album_artists_edited: bool) -> AppResult<Rewrite> {
    // a song on no album shares its art with no other song
    let Some((title, owners)) = tags.album_key() else {
      return Ok(Rewrite {
        tags,
        cover,
        rehomed: false,
        siblings: Vec::new(),
      });
    };
    let albums = self.queries.albums()?;
    // new album artists under the same album title hand the whole album over, as a new cover goes to all its songs
    let rehomed = albums.iter().find(|album| {
      album_artists_edited && Some(album.id) == track.album_id && album.title == title && !owned_by(album, owners)
    });
    let target = rehomed.or_else(|| {
      albums
        .iter()
        .find(|album| album.title == title && owned_by(album, owners))
    });
    let plan = match (album_move(track.album_id, target.map(|album| Some(album.id))), target) {
      (AlbumMove::Joins(_), Some(album)) => Rewrite {
        tags: tags.with_year(album_year(album)).with_release_type(album.release_type),
        cover: album_cover(album),
        rehomed: false,
        siblings: Vec::new(),
      },
      (AlbumMove::Stays, Some(album)) => {
        let art_changed =
          cover != CoverChange::Keep || tags.year() != album_year(album) || tags.release_type() != album.release_type;
        let siblings = if art_changed || rehomed.is_some() {
          let tracks = self.queries.album(album.id)?.tracks;
          tracks.into_iter().filter(|other| other.id != track.id).collect()
        } else {
          Vec::new()
        };
        let cover = match cover {
          CoverChange::Keep => album_cover(album),
          cover => cover,
        };
        Rewrite {
          tags,
          cover,
          rehomed: rehomed.is_some(),
          siblings,
        }
      }
      _ => Rewrite {
        tags,
        cover,
        rehomed: false,
        siblings: Vec::new(),
      },
    };
    Ok(plan)
  }

  fn write(&self, path: &str, plan: &Rewrite) -> AppResult<()> {
    self.songs.write_song(Path::new(path), &plan.tags, &plan.cover)?;
    let share = AlbumShare {
      year: plan.tags.year(),
      release_type: plan.tags.release_type(),
      // the siblings get the owners by name even where this song needs no tag, being its main artist, as theirs differ
      owners: plan.rehomed.then(|| plan.tags.album_owners()),
      cover: &plan.cover,
    };
    for sibling in &plan.siblings {
      self.songs.write_album_art(Path::new(&sibling.path), &share)?;
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
      .ok_or(AppError::NotFound(Missing::Track(id)))
  }
}

/// Whether the album is filed under exactly these artists, in this order.
fn owned_by(album: &Album, owners: &[String]) -> bool {
  album
    .artists
    .iter()
    .map(|artist| artist.name.as_str())
    .eq(owners.iter().map(String::as_str))
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
