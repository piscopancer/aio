//! SQLite storage through Diesel: every table, column and join below is checked against `schema.rs` at compile time.

use std::cmp::Reverse;
use std::collections::HashMap;
use std::hash::Hash;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use diesel::connection::SimpleConnection;
use diesel::dsl::auto_type;
use diesel::dsl::{count, max, sum};
use diesel::helper_types::{IsNull, LeftJoin, LeftJoinOn};
use diesel::prelude::*;
use diesel::result::{ConnectionError, Error as DieselError};
use diesel::sqlite::Sqlite;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use super::schema::{
  album_artists, albums, artists, download_settings, downloads, folders, lyrics, playback_settings, player_session,
  playlist_tracks, playlists, plays, track_artists, track_genres, tracks, window_settings,
};
use crate::application::downloads::DownloadRequest;
use crate::application::error::{AppError, AppResult, Missing};
use crate::application::ports::*;
use crate::application::views::*;
use crate::domain::ids::{AlbumId, ArtistId, DownloadId, FolderId, PlayId, PlaylistId, TrackId};
use crate::domain::library::{FolderPath, LibraryError, LibraryFolders};
use crate::domain::pins::{PinKind, Pins};
use crate::domain::playback::{Crossfade, Listen, PlaybackContext, QueueSnapshot, RepeatMode, Volume};
use crate::domain::playlist::{Playlist as PlaylistAggregate, PlaylistName};
use crate::domain::tags::ReleaseType;

/// Files of `src-tauri/migrations`, compiled in and applied at startup.
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

impl From<DieselError> for AppError {
  fn from(error: DieselError) -> Self {
    match error {
      DieselError::NotFound => Self::NotFound(Missing::Record),
      other => Self::Storage(other.to_string()),
    }
  }
}

impl From<ConnectionError> for AppError {
  fn from(error: ConnectionError) -> Self {
    Self::Storage(error.to_string())
  }
}

/// Stored values are written only after passing domain rules, so failing them again means the file was tampered with.
fn corrupt(what: &str) -> AppError {
  AppError::Storage(format!("stored {what} is invalid"))
}

/// Reads an unsigned domain number back from a signed SQLite integer.
fn stored<T: TryInto<u32>>(value: T) -> AppResult<u32> {
  value.try_into().map_err(|_| corrupt("number"))
}

fn stored_opt(value: Option<i32>) -> AppResult<Option<u32>> {
  value.map(stored).transpose()
}

/// Writes an unsigned domain number into a signed SQLite integer.
fn column(value: u32) -> AppResult<i32> {
  i32::try_from(value).map_err(|_| AppError::Storage(format!("{value} does not fit an integer column")))
}

fn column_opt(value: Option<u32>) -> AppResult<Option<i32>> {
  value.map(column).transpose()
}

fn columns<T>(ids: &[T], raw: impl Fn(&T) -> u32) -> AppResult<Vec<i32>> {
  ids.iter().map(|id| column(raw(id))).collect()
}

/// Totals can outgrow `u32` milliseconds only past 49 days of audio, where showing the cap is good enough.
fn saturated(total: Option<i64>) -> u32 {
  total.map_or(0, |total| u32::try_from(total).unwrap_or(u32::MAX))
}

/// The id of a played song or playlist and when it was last played, none when no play of it counts.
type ScoreRow = (i32, Option<i64>);

/// Scored rows as ids and scores, the highest first, leaving out those without one.
fn ranked<I>(rows: Vec<ScoreRow>, id: impl Fn(u32) -> I) -> AppResult<Vec<(I, u32)>> {
  let mut rows: Vec<(i32, u32)> = rows
    .into_iter()
    .map(|(key, score)| (key, saturated(score)))
    .filter(|(_, score)| *score > 0)
    .collect();
  rows.sort_by_key(|(_, score)| Reverse(*score));
  rows
    .into_iter()
    .map(|(key, score)| Ok((id(stored(key)?), score)))
    .collect()
}

/// Stored spelling of the kind of a playback context, the same words the migration's CHECK allows.
fn context_kind(context: PlaybackContext) -> &'static str {
  match context {
    PlaybackContext::Songs => "songs",
    PlaybackContext::Album { .. } => "album",
    PlaybackContext::Artist { .. } => "artist",
    PlaybackContext::Playlist { .. } => "playlist",
    PlaybackContext::Folder { .. } => "folder",
    PlaybackContext::Recent => "recent",
    PlaybackContext::Added => "added",
  }
}

/// The record a context is of, when it is one.
fn context_id(context: PlaybackContext) -> Option<u32> {
  match context {
    PlaybackContext::Album { id } => Some(id.0),
    PlaybackContext::Artist { id } => Some(id.0),
    PlaybackContext::Playlist { id } => Some(id.0),
    PlaybackContext::Folder { id } => Some(id.0),
    PlaybackContext::Songs | PlaybackContext::Recent | PlaybackContext::Added => None,
  }
}

/// SQLite caps the bound parameters of one statement, so long id lists are sent in parts of this size.
const CHUNK: usize = 500;

/// First value per key, keeping the order in which rows arrive.
fn first_per_key<K: Hash + Eq, V>(rows: Vec<(K, V)>) -> HashMap<K, V> {
  let mut firsts = HashMap::with_capacity(rows.len());
  for (key, value) in rows {
    firsts.entry(key).or_insert(value);
  }
  firsts
}

/// Number of tracks and their total length, grouped by some owner id.
#[derive(Debug, Clone, Copy, Default)]
struct Totals {
  tracks: u32,
  duration_ms: u32,
}

fn totals_by(rows: Vec<(i32, i64, Option<i64>)>) -> AppResult<HashMap<i32, Totals>> {
  rows
    .into_iter()
    .map(|(id, tracks, duration)| {
      let totals = Totals {
        tracks: stored(tracks)?,
        duration_ms: saturated(duration),
      };
      Ok((id, totals))
    })
    .collect()
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = tracks, check_for_backend(Sqlite))]
struct TrackRow {
  id: i32,
  path: String,
  title: String,
  album_id: Option<i32>,
  #[diesel(select_expression = albums::title.nullable())]
  album: Option<String>,
  #[diesel(select_expression = albums::cover.nullable())]
  album_cover: Option<String>,
  /// The cover of the song's own file.
  cover: Option<String>,
  track_no: Option<i32>,
  disc_no: Option<i32>,
  duration_ms: i32,
  #[diesel(select_expression = tracks::folder_id.is_null(), select_expression_type = IsNull<tracks::folder_id>)]
  missing: bool,
  /// Null for a song of no folder, as SQL compares a missing folder with the hidden ones.
  #[diesel(select_expression = in_hidden_folder(), select_expression_type = in_hidden_folder)]
  hidden: Option<bool>,
}

/// Ids of the hidden folders, comparable with a track's folder.
#[auto_type]
fn hidden_folders() -> _ {
  folders::table.filter(folders::hidden).select(folders::id.nullable())
}

/// Tracks of a hidden folder.
#[auto_type]
fn in_hidden_folder() -> _ {
  let hidden: hidden_folders = hidden_folders();
  tracks::folder_id.eq_any(hidden)
}

/// Tracks whose file is in a shown library folder; the others are kept only for the playlists that hold them.
#[auto_type]
fn present() -> _ {
  let hidden: hidden_folders = hidden_folders();
  tracks::folder_id.is_not_null().and(tracks::folder_id.ne_all(hidden))
}

impl TrackRow {
  fn track(self, artists: Vec<ArtistName>) -> AppResult<Track> {
    Ok(Track {
      id: TrackId(stored(self.id)?),
      path: self.path,
      title: self.title,
      artists,
      album_id: self.album_id.map(stored).transpose()?.map(AlbumId),
      album: self.album,
      // the album's cover is the one its songs share; a song of no album shows its own
      cover: if self.album_id.is_some() {
        self.album_cover
      } else {
        self.cover
      },
      track_no: stored_opt(self.track_no)?,
      disc_no: stored_opt(self.disc_no)?,
      duration_ms: stored(self.duration_ms)?,
      presence: if self.missing {
        Presence::Missing
      } else if self.hidden == Some(true) {
        Presence::Hidden
      } else {
        Presence::Present
      },
    })
  }
}

/// The credited artists of each of the tracks, in their order.
fn credits(conn: &mut SqliteConnection, ids: &[i32]) -> AppResult<HashMap<i32, Vec<ArtistName>>> {
  let mut credits: HashMap<i32, Vec<ArtistName>> = HashMap::with_capacity(ids.len());
  for part in ids.chunks(CHUNK) {
    let rows: Vec<(i32, i32, String)> = track_artists::table
      .inner_join(artists::table)
      .filter(track_artists::track_id.eq_any(part))
      .order((track_artists::track_id, track_artists::position))
      .select((track_artists::track_id, artists::id, artists::name))
      .load(conn)?;
    for (track, id, name) in rows {
      let artist = ArtistName {
        id: ArtistId(stored(id)?),
        name,
      };
      credits.entry(track).or_default().push(artist);
    }
  }
  Ok(credits)
}

/// The artists each of the albums is filed under, in their order.
fn album_credits(conn: &mut SqliteConnection, ids: &[i32]) -> AppResult<HashMap<i32, Vec<ArtistName>>> {
  let mut credits: HashMap<i32, Vec<ArtistName>> = HashMap::with_capacity(ids.len());
  for part in ids.chunks(CHUNK) {
    let rows: Vec<(i32, i32, String)> = album_artists::table
      .inner_join(artists::table)
      .filter(album_artists::album_id.eq_any(part))
      .order((album_artists::album_id, album_artists::position))
      .select((album_artists::album_id, artists::id, artists::name))
      .load(conn)?;
    for (album, id, name) in rows {
      let artist = ArtistName {
        id: ArtistId(stored(id)?),
        name,
      };
      credits.entry(album).or_default().push(artist);
    }
  }
  Ok(credits)
}

fn tracks_from(conn: &mut SqliteConnection, rows: Vec<TrackRow>) -> AppResult<Vec<Track>> {
  let ids: Vec<i32> = rows.iter().map(|row| row.id).collect();
  let credits = credits(conn, &ids)?;
  rows
    .into_iter()
    .map(|row| {
      let artists = credits.get(&row.id).cloned().unwrap_or_default();
      row.track(artists)
    })
    .collect()
}

/// Tracks with their album, if any, the source of every `TrackRow`.
type TrackSource = LeftJoin<tracks::table, albums::table>;

fn track_source() -> TrackSource {
  tracks::table.left_join(albums::table)
}

/// Position of a song's main artist among its credits.
const MAIN_CREDIT: i32 = 0;

/// The join of a track to its first credit.
type MainCredit =
  diesel::dsl::And<diesel::dsl::Eq<track_artists::track_id, tracks::id>, diesel::dsl::Eq<track_artists::position, i32>>;

/// Tracks with their album and main artist, for the lists in `library_order`; the main artist only sorts them.
type LibrarySource = LeftJoinOn<
  LeftJoinOn<TrackSource, track_artists::table, MainCredit>,
  artists::table,
  diesel::dsl::Eq<artists::id, track_artists::artist_id>,
>;

fn library_source() -> LibrarySource {
  track_source()
    .left_join(
      track_artists::table.on(
        track_artists::track_id
          .eq(tracks::id)
          .and(track_artists::position.eq(MAIN_CREDIT)),
      ),
    )
    .left_join(artists::table.on(artists::id.eq(track_artists::artist_id)))
}

/// Order of library-wide track lists: by artist, the songs credited to nobody last, then album, then position on the
/// album.
type LibraryOrder = (
  IsNull<artists::name>,
  artists::name,
  albums::year,
  albums::title,
  tracks::disc_no,
  tracks::track_no,
  tracks::title,
);

fn library_order() -> LibraryOrder {
  (
    artists::name.is_null(),
    artists::name,
    albums::year,
    albums::title,
    tracks::disc_no,
    tracks::track_no,
    tracks::title,
  )
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = albums, check_for_backend(Sqlite))]
struct AlbumRow {
  id: i32,
  title: String,
  year: Option<i32>,
  cover: Option<String>,
  pin: Option<i32>,
  release_type: Option<String>,
}

/// Which albums a view lists, each with its own order.
enum AlbumScope {
  All,
  One(i32),
  ByArtist(i32),
  /// Albums of other artists holding songs that credit this one.
  AppearsOn(i32),
}

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = tracks, treat_none_as_null = true)]
struct NewTrack<'a> {
  folder_id: Option<i32>,
  path: &'a str,
  title: &'a str,
  album_id: Option<i32>,
  track_no: Option<i32>,
  disc_no: Option<i32>,
  duration_ms: i32,
  modified: i64,
  cover: Option<&'a str>,
}

#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = playback_settings, check_for_backend(Sqlite))]
struct SettingsRow {
  id: i32,
  volume: i32,
  shuffle: bool,
  repeat: String,
  fades: bool,
  crossfade_seconds: i32,
  discord_status: bool,
}

/// The settings table holds exactly one row, with this id.
const SETTINGS_ID: i32 = 1;

const REPEAT_MODES: [RepeatMode; 3] = [RepeatMode::Off, RepeatMode::All, RepeatMode::One];

/// Stored spelling of a repeat mode, the same words the migration's CHECK allows.
fn repeat_text(mode: RepeatMode) -> &'static str {
  match mode {
    RepeatMode::Off => "off",
    RepeatMode::All => "all",
    RepeatMode::One => "one",
  }
}

fn parse_repeat(text: &str) -> Option<RepeatMode> {
  REPEAT_MODES.into_iter().find(|mode| repeat_text(*mode) == text)
}

/// SQLite-backed storage for folders, the track index, playlists, settings and all read views.
pub struct SqliteStore(Mutex<SqliteConnection>);

impl SqliteStore {
  pub fn open(path: &Path) -> AppResult<Self> {
    let url = path
      .to_str()
      .ok_or_else(|| AppError::Storage(format!("database path is not UTF-8: {}", path.display())))?;
    Self::init(SqliteConnection::establish(url)?)
  }

  pub fn in_memory() -> AppResult<Self> {
    Self::init(SqliteConnection::establish(":memory:")?)
  }

  fn init(mut conn: SqliteConnection) -> AppResult<Self> {
    conn.batch_execute("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
    conn
      .run_pending_migrations(MIGRATIONS)
      .map_err(|error| AppError::Storage(format!("migration failed: {error}")))?;
    Ok(Self(Mutex::new(conn)))
  }

  fn conn(&self) -> MutexGuard<'_, SqliteConnection> {
    // a poisoned lock only means another call panicked mid-query; SQLite itself stays consistent
    self.0.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
  }

  /// Runs `work` in one write transaction, so a failure leaves nothing half-written.
  fn write<T>(&self, work: impl FnOnce(&mut SqliteConnection) -> AppResult<T>) -> AppResult<T> {
    self.conn().immediate_transaction(work)
  }

  /// Library folders with their track counts, all of them or only `only`.
  fn folder_views(&self, only: Option<i32>) -> AppResult<Vec<Folder>> {
    let conn = &mut *self.conn();
    let mut query = folders::table
      .select((folders::id, folders::path, folders::hidden))
      .order(folders::path)
      .into_boxed();
    if let Some(id) = only {
      query = query.filter(folders::id.eq(id));
    }
    let rows: Vec<(i32, String, bool)> = query.load(conn)?;
    let counts: HashMap<i32, i64> = tracks::table
      .filter(tracks::folder_id.is_not_null())
      .group_by(tracks::folder_id)
      .select((tracks::folder_id.assume_not_null(), count(tracks::id)))
      .load(conn)?
      .into_iter()
      .collect();
    rows
      .into_iter()
      .map(|(id, path, hidden)| {
        Ok(Folder {
          id: FolderId(stored(id)?),
          path,
          track_count: stored(counts.get(&id).copied().unwrap_or_default())?,
          hidden,
          // only the library service knows which folder is the system one
          system: false,
        })
      })
      .collect()
  }

  fn load_albums(&self, scope: AlbumScope) -> AppResult<Vec<Album>> {
    let conn = &mut *self.conn();
    let query = albums::table.select(AlbumRow::as_select()).into_boxed();
    let owned_by = |id: i32| {
      album_artists::table
        .filter(album_artists::artist_id.eq(id))
        .select(album_artists::album_id)
    };
    let query = match scope {
      AlbumScope::All => query.order(albums::title),
      AlbumScope::One(id) => query.filter(albums::id.eq(id)),
      AlbumScope::ByArtist(id) => query
        .filter(albums::id.eq_any(owned_by(id)))
        .order((albums::year.desc(), albums::title)),
      AlbumScope::AppearsOn(id) => query
        .filter(albums::id.ne_all(owned_by(id)))
        .filter(
          albums::id.eq_any(
            tracks::table
              .inner_join(track_artists::table)
              .filter(track_artists::artist_id.eq(id))
              .filter(present())
              .filter(tracks::album_id.is_not_null())
              .select(tracks::album_id.assume_not_null()),
          ),
        )
        .order((albums::year.desc(), albums::title)),
    };
    let rows = query.load(conn)?;
    let ids: Vec<i32> = rows.iter().map(|row| row.id).collect();
    let mut owners = album_credits(conn, &ids)?;
    let totals = totals_by(
      tracks::table
        .filter(present())
        .filter(tracks::album_id.is_not_null())
        .group_by(tracks::album_id)
        .select((
          tracks::album_id.assume_not_null(),
          count(tracks::id),
          sum(tracks::duration_ms),
        ))
        .load(conn)?,
    )?;
    let listing = !matches!(scope, AlbumScope::One(_));
    rows
      .into_iter()
      // an album whose songs all left the library stays only for the playlists that hold them
      .filter(|row| !listing || totals.contains_key(&row.id))
      .map(|row| {
        let Totals { tracks, duration_ms } = totals.get(&row.id).copied().unwrap_or_default();
        Ok(Album {
          id: AlbumId(stored(row.id)?),
          title: row.title,
          artists: owners.remove(&row.id).unwrap_or_default(),
          year: stored_opt(row.year)?,
          release_type: row.release_type.as_deref().and_then(ReleaseType::parse),
          cover: row.cover,
          track_count: tracks,
          duration_ms,
          pin: stored_opt(row.pin)?,
        })
      })
      .collect()
  }

  /// Artists credited on at least one track, as its main artist or a guest, or owning an album.
  fn load_artists(&self, only: Option<i32>) -> AppResult<Vec<Artist>> {
    let conn = &mut *self.conn();
    let mut query = artists::table
      .select((artists::id, artists::name, artists::pin))
      .order(artists::name)
      .into_boxed();
    if let Some(id) = only {
      query = query.filter(artists::id.eq(id));
    }
    let rows: Vec<(i32, String, Option<i32>)> = query.load(conn)?;
    let track_counts: HashMap<i32, i64> = track_artists::table
      .inner_join(tracks::table)
      .filter(present())
      .group_by(track_artists::artist_id)
      .select((track_artists::artist_id, count(tracks::id)))
      .load(conn)?
      .into_iter()
      .collect();
    let album_counts: HashMap<i32, i64> = album_artists::table
      .group_by(album_artists::artist_id)
      .select((album_artists::artist_id, count(album_artists::album_id)))
      .load(conn)?
      .into_iter()
      .collect();
    // the newest album with art stands for the artist, or else the newest one they sing on as a guest, or else a song
    // of theirs that is on no album
    let covers = first_per_key(
      album_artists::table
        .inner_join(albums::table)
        .filter(albums::cover.is_not_null())
        .order(albums::year.desc())
        .select((album_artists::artist_id, albums::cover.assume_not_null()))
        .load::<(i32, String)>(conn)?,
    );
    let guest_covers = first_per_key(
      track_artists::table
        .inner_join(tracks::table.inner_join(albums::table))
        .filter(albums::cover.is_not_null())
        .order(albums::year.desc())
        .select((track_artists::artist_id, albums::cover.assume_not_null()))
        .load::<(i32, String)>(conn)?,
    );
    let own_covers = first_per_key(
      track_artists::table
        .inner_join(tracks::table)
        .filter(tracks::album_id.is_null())
        .filter(tracks::cover.is_not_null())
        .select((track_artists::artist_id, tracks::cover.assume_not_null()))
        .load::<(i32, String)>(conn)?,
    );
    // an artist counts once they sing on a song or own an album, as the album artist of a compilation may do neither else
    rows
      .into_iter()
      .filter(|(id, ..)| track_counts.contains_key(id) || album_counts.contains_key(id))
      .map(|(id, name, pin)| {
        let track_count = track_counts.get(&id).copied().unwrap_or_default();
        Ok(Artist {
          id: ArtistId(stored(id)?),
          name,
          cover: covers
            .get(&id)
            .or_else(|| guest_covers.get(&id))
            .or_else(|| own_covers.get(&id))
            .cloned(),
          album_count: stored(album_counts.get(&id).copied().unwrap_or_default())?,
          track_count: stored(track_count)?,
          pin: stored_opt(pin)?,
        })
      })
      .collect()
  }

  fn load_playlists(&self, only: Option<i32>) -> AppResult<Vec<Playlist>> {
    let conn = &mut *self.conn();
    let mut query = playlists::table
      .select((
        playlists::id,
        playlists::name,
        playlists::cover,
        playlists::created_at,
        playlists::played_at,
        playlists::pin,
      ))
      .order((playlists::created_at.desc(), playlists::id.desc()))
      .into_boxed();
    if let Some(id) = only {
      query = query.filter(playlists::id.eq(id));
    }
    let rows: Vec<PlaylistRow> = query.load(conn)?;
    let totals = totals_by(
      playlist_tracks::table
        .inner_join(tracks::table)
        .group_by(playlist_tracks::playlist_id)
        .select((
          playlist_tracks::playlist_id,
          count(tracks::id),
          sum(tracks::duration_ms),
        ))
        .load(conn)?,
    )?;
    // the art of the first track that has any stands for the playlist: its album's, or its own when it is on none
    let covers = first_per_key(
      playlist_tracks::table
        .inner_join(tracks::table.left_join(albums::table))
        .order((playlist_tracks::playlist_id, playlist_tracks::position))
        .select((
          playlist_tracks::playlist_id,
          tracks::album_id,
          albums::cover.nullable(),
          tracks::cover,
        ))
        .load::<(i32, Option<i32>, Option<String>, Option<String>)>(conn)?
        .into_iter()
        .filter_map(|(playlist, album, album_cover, own)| {
          Some((playlist, if album.is_some() { album_cover } else { own }?))
        })
        .collect(),
    );
    rows
      .into_iter()
      .map(|(id, name, own_cover, created_at, played_at, pin)| {
        let Totals { tracks, duration_ms } = totals.get(&id).copied().unwrap_or_default();
        Ok(Playlist {
          id: PlaylistId(stored(id)?),
          name,
          cover: own_cover.clone().or_else(|| covers.get(&id).cloned()),
          own_cover,
          track_count: tracks,
          duration_ms,
          created_at: stored(created_at)?,
          played_at: played_at.map(stored).transpose()?,
          pin: stored_opt(pin)?,
        })
      })
      .collect()
  }
}

/// Id, name, own cover, creation and last play time, and pin of a stored playlist.
type PlaylistRow = (i32, String, Option<String>, i64, Option<i64>, Option<i32>);

/// The one row a lookup by id gave, or `missing` when there is none.
fn single<T>(rows: Vec<T>, missing: Missing) -> AppResult<T> {
  rows.into_iter().next().ok_or(AppError::NotFound(missing))
}

fn upsert_artist(conn: &mut SqliteConnection, name: &str) -> AppResult<i32> {
  diesel::insert_or_ignore_into(artists::table)
    .values(artists::name.eq(name))
    .execute(conn)?;
  Ok(
    artists::table
      .filter(artists::name.eq(name))
      .select(artists::id)
      .first(conn)?,
  )
}

/// How the tags of a track treat the year and cover its album already has.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AlbumArt {
  /// A scan only fills in what is still missing, since the songs of one album may disagree.
  Fill,
  /// An edit wrote the album's year and cover into the file, so the file is right.
  Replace,
}

/// The album with this title filed under exactly these artists, in this order.
fn find_album(conn: &mut SqliteConnection, title: &str, owner_ids: &[i32]) -> AppResult<Option<i32>> {
  // the title narrows the search, and the whole list of album artists settles it
  let candidates: Vec<i32> = albums::table
    .filter(albums::title.eq(title))
    .select(albums::id)
    .load(conn)?;
  for id in candidates {
    let owners: Vec<i32> = album_artists::table
      .filter(album_artists::album_id.eq(id))
      .order(album_artists::position)
      .select(album_artists::artist_id)
      .load(conn)?;
    if owners == owner_ids {
      return Ok(Some(id));
    }
  }
  Ok(None)
}

/// Finds or creates the album of a scanned track and settles its year, cover and release type by `art`.
fn upsert_album(
  conn: &mut SqliteConnection,
  track: &ScannedTrack,
  title: &str,
  owner_ids: &[i32],
  art: AlbumArt,
) -> AppResult<i32> {
  let year = column_opt(track.year)?;
  let release_type = track.release_type.map(ReleaseType::tag);
  let Some(id) = find_album(conn, title, owner_ids)? else {
    let id = diesel::insert_into(albums::table)
      .values((
        albums::title.eq(title),
        albums::year.eq(year),
        albums::cover.eq(&track.cover),
        albums::release_type.eq(release_type),
      ))
      .returning(albums::id)
      .get_result(conn)?;
    let rows = (0..)
      .zip(owner_ids)
      .map(|(position, artist_id)| {
        (
          album_artists::album_id.eq(id),
          album_artists::artist_id.eq(*artist_id),
          album_artists::position.eq(position),
        )
      })
      .collect::<Vec<_>>();
    if !rows.is_empty() {
      diesel::insert_into(album_artists::table).values(&rows).execute(conn)?;
    }
    return Ok(id);
  };
  if art == AlbumArt::Replace {
    diesel::update(albums::table.find(id))
      .set((
        albums::year.eq(year),
        albums::cover.eq(&track.cover),
        albums::release_type.eq(release_type),
      ))
      .execute(conn)?;
    return Ok(id);
  }
  if release_type.is_some() {
    diesel::update(albums::table.find(id).filter(albums::release_type.is_null()))
      .set(albums::release_type.eq(release_type))
      .execute(conn)?;
  }
  if year.is_some() {
    diesel::update(albums::table.find(id).filter(albums::year.is_null()))
      .set(albums::year.eq(year))
      .execute(conn)?;
  }
  if track.cover.is_some() {
    diesel::update(albums::table.find(id).filter(albums::cover.is_null()))
      .set(albums::cover.eq(&track.cover))
      .execute(conn)?;
  }
  Ok(id)
}

/// Ids of the credited artists, in order, and of the album a scanned track belongs to, if it names one, creating them
/// when new; the album artists are kept only for an album.
fn track_owners(
  conn: &mut SqliteConnection,
  track: &ScannedTrack,
  art: AlbumArt,
) -> AppResult<(Vec<i32>, Option<i32>)> {
  let artist_ids = track
    .artists
    .iter()
    .flat_map(|artists| artists.names())
    .map(|name| upsert_artist(conn, name))
    .collect::<AppResult<Vec<_>>>()?;
  let Some(title) = &track.album else {
    return Ok((artist_ids, None));
  };
  let owner_ids = track
    .album_artists
    .iter()
    .flat_map(|owners| owners.names())
    .map(|name| upsert_artist(conn, name))
    .collect::<AppResult<Vec<_>>>()?;
  Ok((artist_ids, Some(upsert_album(conn, track, title, &owner_ids, art)?)))
}

/// Replaces the credited artists of a track with these, in this order.
fn set_credits(conn: &mut SqliteConnection, track_id: i32, artist_ids: &[i32]) -> AppResult<()> {
  diesel::delete(track_artists::table.filter(track_artists::track_id.eq(track_id))).execute(conn)?;
  let rows = (0..)
    .zip(artist_ids)
    .map(|(position, artist_id)| {
      (
        track_artists::track_id.eq(track_id),
        track_artists::artist_id.eq(*artist_id),
        track_artists::position.eq(position),
      )
    })
    .collect::<Vec<_>>();
  if !rows.is_empty() {
    diesel::insert_into(track_artists::table).values(&rows).execute(conn)?;
  }
  Ok(())
}

/// Replaces the genres of a track with these, in this order.
fn set_genres(conn: &mut SqliteConnection, track_id: i32, genres: &[String]) -> AppResult<()> {
  diesel::delete(track_genres::table.filter(track_genres::track_id.eq(track_id))).execute(conn)?;
  let rows = (0..)
    .zip(genres)
    .map(|(position, name)| {
      (
        track_genres::track_id.eq(track_id),
        track_genres::name.eq(name),
        track_genres::position.eq(position),
      )
    })
    .collect::<Vec<_>>();
  if !rows.is_empty() {
    diesel::insert_into(track_genres::table).values(&rows).execute(conn)?;
  }
  Ok(())
}

/// Drops missing tracks no playlist holds, then albums without tracks and artists credited on no track and owning no
/// album.
fn remove_orphans(conn: &mut SqliteConnection) -> AppResult<()> {
  diesel::delete(
    tracks::table
      .filter(tracks::folder_id.is_null())
      .filter(tracks::id.ne_all(playlist_tracks::table.select(playlist_tracks::track_id))),
  )
  .execute(conn)?;
  let held = tracks::table
    .filter(tracks::album_id.is_not_null())
    .select(tracks::album_id.assume_not_null());
  diesel::delete(albums::table.filter(albums::id.ne_all(held))).execute(conn)?;
  diesel::delete(
    artists::table
      .filter(artists::id.ne_all(track_artists::table.select(track_artists::artist_id)))
      .filter(artists::id.ne_all(album_artists::table.select(album_artists::artist_id))),
  )
  .execute(conn)?;
  Ok(())
}

impl FolderRepository for SqliteStore {
  fn load_folders(&self) -> AppResult<LibraryFolders> {
    let paths: Vec<String> = folders::table
      .select(folders::path)
      .order(folders::path)
      .load(&mut *self.conn())?;
    let folders = paths
      .iter()
      .map(|p| FolderPath::new(p).map_err(|_| corrupt("folder")))
      .collect::<AppResult<_>>()?;
    Ok(LibraryFolders::restore(folders))
  }

  fn save_folders(&self, library: &LibraryFolders) -> AppResult<()> {
    let kept: Vec<String> = library.folders().iter().map(FolderPath::display).collect();
    self.write(|conn| {
      for path in &kept {
        diesel::insert_or_ignore_into(folders::table)
          .values(folders::path.eq(path))
          .execute(conn)?;
      }
      let leaving: Vec<(i32, String)> = folders::table
        .filter(folders::path.ne_all(&kept))
        .select((folders::id, folders::path))
        .load(conn)?;
      // a folder merged into one holding it hands its songs over, so they keep their ids, playlists and lyrics
      for (id, path) in leaving {
        let Some(holder) = FolderPath::new(&path)
          .ok()
          .and_then(|folder| library.containing(&folder).cloned())
        else {
          continue;
        };
        let holder_id: i32 = folders::table
          .filter(folders::path.eq(holder.display()))
          .select(folders::id)
          .first(conn)?;
        diesel::update(tracks::table.filter(tracks::folder_id.eq(id)))
          .set(tracks::folder_id.eq(holder_id))
          .execute(conn)?;
      }
      diesel::delete(folders::table.filter(folders::path.ne_all(&kept))).execute(conn)?;
      remove_orphans(conn)
    })
  }

  fn set_folder_hidden(&self, folder: &FolderPath, hidden: bool) -> AppResult<()> {
    let changed = diesel::update(folders::table.filter(folders::path.eq(folder.display())))
      .set(folders::hidden.eq(hidden))
      .execute(&mut *self.conn())?;
    if changed == 0 {
      return Err(LibraryError::NotInLibrary.into());
    }
    Ok(())
  }
}

/// Id of a pinned row, as the pin queries read it.
#[derive(QueryableByName)]
struct PinnedRow {
  #[diesel(sql_type = diesel::sql_types::Integer)]
  id: i32,
}

/// The table that holds the rows of a kind of pin, each with its own `pin` column.
fn pin_table(kind: PinKind) -> &'static str {
  match kind {
    PinKind::Playlist => "playlists",
    PinKind::Album => "albums",
    PinKind::Artist => "artists",
  }
}

impl PinStore for SqliteStore {
  fn load_pins(&self, kind: PinKind) -> AppResult<Pins> {
    let rows: Vec<PinnedRow> = diesel::sql_query(format!(
      "SELECT id FROM {} WHERE pin IS NOT NULL ORDER BY pin",
      pin_table(kind)
    ))
    .load(&mut *self.conn())?;
    rows
      .into_iter()
      .map(|row| stored(row.id))
      .collect::<AppResult<Vec<_>>>()
      .map(Pins::new)
  }

  fn save_pins(&self, kind: PinKind, pins: &Pins) -> AppResult<()> {
    let table = pin_table(kind);
    let ids = columns(pins.ids(), |id| *id)?;
    self.write(|conn| {
      diesel::sql_query(format!("UPDATE {table} SET pin = NULL WHERE pin IS NOT NULL")).execute(conn)?;
      for (place, id) in (0..).zip(ids) {
        diesel::sql_query(format!("UPDATE {table} SET pin = ? WHERE id = ?"))
          .bind::<diesel::sql_types::Integer, _>(place)
          .bind::<diesel::sql_types::Integer, _>(id)
          .execute(conn)?;
      }
      Ok(())
    })
  }
}

impl PlaylistRepository for SqliteStore {
  fn create_playlist(&self, name: &PlaylistName, cover: Option<&str>) -> AppResult<PlaylistId> {
    let id: i32 = diesel::insert_into(playlists::table)
      .values((playlists::name.eq(name.as_str()), playlists::cover.eq(cover)))
      .returning(playlists::id)
      .get_result(&mut *self.conn())?;
    Ok(PlaylistId(stored(id)?))
  }

  fn load_playlist(&self, id: PlaylistId) -> AppResult<PlaylistAggregate> {
    let conn = &mut *self.conn();
    let key = column(id.0)?;
    let (name, cover): (String, Option<String>) = playlists::table
      .find(key)
      .select((playlists::name, playlists::cover))
      .first(conn)
      .optional()?
      .ok_or(AppError::NotFound(Missing::Playlist(id)))?;
    let name = PlaylistName::new(&name).map_err(|_| corrupt("playlist name"))?;
    let tracks = playlist_tracks::table
      .filter(playlist_tracks::playlist_id.eq(key))
      .order(playlist_tracks::position)
      .select(playlist_tracks::track_id)
      .load::<i32>(conn)?
      .into_iter()
      .map(|track| Ok(TrackId(stored(track)?)))
      .collect::<AppResult<_>>()?;
    Ok(PlaylistAggregate::restore(id, name, cover, tracks))
  }

  fn save_playlist(&self, playlist: &PlaylistAggregate) -> AppResult<()> {
    let key = column(playlist.id().0)?;
    let entries = (0..)
      .zip(columns(playlist.tracks(), |track| track.0)?)
      .map(|(position, track)| {
        (
          playlist_tracks::playlist_id.eq(key),
          playlist_tracks::track_id.eq(track),
          playlist_tracks::position.eq(position),
        )
      })
      .collect::<Vec<_>>();
    self.write(|conn| {
      let renamed = diesel::update(playlists::table.find(key))
        .set((
          playlists::name.eq(playlist.name().as_str()),
          playlists::cover.eq(playlist.cover()),
        ))
        .execute(conn)?;
      if renamed == 0 {
        return Err(AppError::NotFound(Missing::Playlist(playlist.id())));
      }
      diesel::delete(playlist_tracks::table.filter(playlist_tracks::playlist_id.eq(key))).execute(conn)?;
      for part in entries.chunks(CHUNK) {
        diesel::insert_into(playlist_tracks::table).values(part).execute(conn)?;
      }
      remove_orphans(conn)
    })
  }

  fn delete_playlist(&self, id: PlaylistId) -> AppResult<()> {
    let key = column(id.0)?;
    self.write(|conn| match diesel::delete(playlists::table.find(key)).execute(conn)? {
      0 => Err(AppError::NotFound(Missing::Playlist(id))),
      _ => remove_orphans(conn),
    })
  }

  fn mark_playlist_played(&self, id: PlaylistId, at: i64) -> AppResult<()> {
    let key = column(id.0)?;
    self.write(|conn| {
      match diesel::update(playlists::table.find(key))
        .set(playlists::played_at.eq(at))
        .execute(conn)?
      {
        0 => Err(AppError::NotFound(Missing::Playlist(id))),
        _ => Ok(()),
      }
    })
  }
}

impl TrackIndex for SqliteStore {
  fn indexed_files(&self, folder: &FolderPath) -> AppResult<Vec<AudioFile>> {
    let rows: Vec<(String, i64)> = tracks::table
      .inner_join(folders::table)
      .filter(folders::path.eq(folder.display()))
      .select((tracks::path, tracks::modified))
      .load(&mut *self.conn())?;
    Ok(
      rows
        .into_iter()
        .map(|(path, modified)| AudioFile { path, modified })
        .collect(),
    )
  }

  fn index_tracks(&self, folder: &FolderPath, scanned: &[ScannedTrack]) -> AppResult<()> {
    if scanned.is_empty() {
      return Ok(());
    }
    self.write(|conn| {
      let folder_id: i32 = folders::table
        .filter(folders::path.eq(folder.display()))
        .select(folders::id)
        .first(conn)
        .optional()?
        .ok_or_else(|| AppError::NotFound(Missing::FolderPath(folder.display().to_string())))?;
      for track in scanned {
        let (artist_ids, album_id) = track_owners(conn, track, AlbumArt::Fill)?;
        let row = NewTrack {
          folder_id: Some(folder_id),
          path: &track.path,
          title: &track.title,
          album_id,
          track_no: column_opt(track.track_no)?,
          disc_no: column_opt(track.disc_no)?,
          duration_ms: column(track.duration_ms)?,
          modified: track.modified,
          cover: track.cover.as_deref(),
        };
        let track_id: i32 = diesel::insert_into(tracks::table)
          .values(&row)
          .on_conflict(tracks::path)
          .do_update()
          .set(&row)
          .returning(tracks::id)
          .get_result(conn)?;
        set_credits(conn, track_id, &artist_ids)?;
        set_genres(conn, track_id, &track.genres)?;
      }
      remove_orphans(conn)
    })
  }

  fn track_at(&self, path: &str) -> AppResult<TrackId> {
    let id: i32 = tracks::table
      .filter(tracks::path.eq(path))
      .select(tracks::id)
      .first(&mut *self.conn())
      .optional()?
      .ok_or_else(|| AppError::NotFound(Missing::TrackFile(path.to_owned())))?;
    Ok(TrackId(stored(id)?))
  }

  fn retag_tracks(&self, scanned: &[ScannedTrack]) -> AppResult<()> {
    self.write(|conn| {
      for track in scanned {
        let (artist_ids, album_id) = track_owners(conn, track, AlbumArt::Replace)?;
        let track_id: i32 = diesel::update(tracks::table.filter(tracks::path.eq(&track.path)))
          .set((
            tracks::title.eq(&track.title),
            tracks::album_id.eq(album_id),
            tracks::track_no.eq(column_opt(track.track_no)?),
            tracks::disc_no.eq(column_opt(track.disc_no)?),
            tracks::duration_ms.eq(column(track.duration_ms)?),
            tracks::modified.eq(track.modified),
            tracks::cover.eq(&track.cover),
          ))
          .returning(tracks::id)
          .get_result(conn)?;
        set_credits(conn, track_id, &artist_ids)?;
        set_genres(conn, track_id, &track.genres)?;
      }
      remove_orphans(conn)
    })
  }

  fn drop_files(&self, paths: &[String]) -> AppResult<()> {
    self.write(|conn| {
      // the tracks go missing, and only those no playlist holds are deleted
      for part in paths.chunks(CHUNK) {
        diesel::update(tracks::table.filter(tracks::path.eq_any(part)))
          .set(tracks::folder_id.eq(None::<i32>))
          .execute(conn)?;
      }
      remove_orphans(conn)
    })
  }
}

impl LibraryQueries for SqliteStore {
  fn folders(&self) -> AppResult<Vec<Folder>> {
    self.folder_views(None)
  }

  fn folder(&self, id: FolderId) -> AppResult<FolderDetail> {
    let key = column(id.0)?;
    let folder = single(self.folder_views(Some(key))?, Missing::Folder(id))?;
    let conn = &mut *self.conn();
    let rows = library_source()
      .filter(tracks::folder_id.eq(key))
      .filter(present())
      .order(library_order())
      .select(TrackRow::as_select())
      .load(conn)?;
    let tracks = tracks_from(conn, rows)?;
    Ok(FolderDetail { folder, tracks })
  }

  fn tracks(&self) -> AppResult<Vec<Track>> {
    let conn = &mut *self.conn();
    let rows = library_source()
      .filter(present())
      .select(TrackRow::as_select())
      .order(library_order())
      .load(conn)?;
    tracks_from(conn, rows)
  }

  fn tracks_by_ids(&self, ids: &[TrackId]) -> AppResult<Vec<Track>> {
    let keys = columns(ids, |id| id.0)?;
    let mut found = HashMap::with_capacity(keys.len());
    {
      let conn = &mut *self.conn();
      for part in keys.chunks(CHUNK) {
        let rows = track_source()
          .filter(tracks::id.eq_any(part))
          .filter(present())
          .select(TrackRow::as_select())
          .load(conn)?;
        for track in tracks_from(conn, rows)? {
          found.insert(track.id, track);
        }
      }
    }
    // keeps the requested order and repeats, skipping ids that no longer exist
    Ok(ids.iter().filter_map(|id| found.get(id).cloned()).collect())
  }

  fn albums(&self) -> AppResult<Vec<Album>> {
    self.load_albums(AlbumScope::All)
  }

  fn album(&self, id: AlbumId) -> AppResult<AlbumDetail> {
    let key = column(id.0)?;
    let album = single(self.load_albums(AlbumScope::One(key))?, Missing::Album(id))?;
    let conn = &mut *self.conn();
    let rows = track_source()
      .filter(tracks::album_id.eq(key))
      .filter(present())
      .order((tracks::disc_no, tracks::track_no, tracks::title))
      .select(TrackRow::as_select())
      .load(conn)?;
    let tracks = tracks_from(conn, rows)?;
    Ok(AlbumDetail { album, tracks })
  }

  fn artists(&self) -> AppResult<Vec<Artist>> {
    self.load_artists(None)
  }

  fn genres(&self) -> AppResult<Vec<Genre>> {
    let song_count = count(track_genres::track_id);
    let rows: Vec<(String, i64)> = track_genres::table
      .inner_join(tracks::table)
      .filter(present())
      .group_by(track_genres::name)
      .select((track_genres::name, song_count))
      .order((song_count.desc(), track_genres::name))
      .load(&mut *self.conn())?;
    rows
      .into_iter()
      .map(|(name, track_count)| {
        Ok(Genre {
          name,
          track_count: stored(track_count)?,
        })
      })
      .collect()
  }

  fn artist(&self, id: ArtistId) -> AppResult<ArtistDetail> {
    let key = column(id.0)?;
    let artist = single(self.load_artists(Some(key))?, Missing::Artist(id))?;
    let albums = self.load_albums(AlbumScope::ByArtist(key))?;
    let appears_on = self.load_albums(AlbumScope::AppearsOn(key))?;
    let conn = &mut *self.conn();
    // under its own name, as the main credit the songs are sorted by is joined from the same table
    let credits = diesel::alias!(track_artists as credits);
    let credited = credits
      .filter(credits.field(track_artists::artist_id).eq(key))
      .select(credits.field(track_artists::track_id));
    let rows = library_source()
      .filter(tracks::id.eq_any(credited))
      .filter(present())
      .order(library_order())
      .select(TrackRow::as_select())
      .load(conn)?;
    let tracks = tracks_from(conn, rows)?;
    Ok(ArtistDetail {
      artist,
      albums,
      appears_on,
      tracks,
    })
  }

  fn playlists(&self) -> AppResult<Vec<Playlist>> {
    self.load_playlists(None)
  }

  fn playlists_with_track(&self, track: TrackId) -> AppResult<Vec<PlaylistId>> {
    playlist_tracks::table
      .filter(playlist_tracks::track_id.eq(column(track.0)?))
      .select(playlist_tracks::playlist_id)
      .distinct()
      .load::<i32>(&mut *self.conn())?
      .into_iter()
      .map(|id| Ok(PlaylistId(stored(id)?)))
      .collect()
  }

  fn scores(&self) -> AppResult<Scores> {
    let conn = &mut *self.conn();
    let tracks: Vec<ScoreRow> = plays::table
      .filter(plays::counted)
      .group_by(plays::track_id)
      .select((plays::track_id, max(plays::started_at)))
      .load(conn)?;
    let playlists: Vec<ScoreRow> = plays::table
      .filter(plays::counted)
      .filter(plays::context_kind.eq(context_kind(PlaybackContext::Playlist { id: PlaylistId(0) })))
      .filter(plays::context_id.is_not_null())
      .group_by(plays::context_id)
      .select((plays::context_id.assume_not_null(), max(plays::started_at)))
      .load(conn)?;
    Ok(Scores {
      tracks: ranked(tracks, TrackId)?,
      playlists: ranked(playlists, PlaylistId)?,
    })
  }

  fn newest_tracks(&self, limit: u32) -> AppResult<Vec<Track>> {
    let conn = &mut *self.conn();
    let rows = track_source()
      .filter(present())
      .select(TrackRow::as_select())
      .order((tracks::modified.desc(), tracks::id.desc()))
      .limit(i64::from(limit))
      .load(conn)?;
    tracks_from(conn, rows)
  }

  fn playlist(&self, id: PlaylistId) -> AppResult<PlaylistDetail> {
    let key = column(id.0)?;
    let playlist = single(self.load_playlists(Some(key))?, Missing::Playlist(id))?;
    let conn = &mut *self.conn();
    let rows = playlist_tracks::table
      .inner_join(track_source())
      .filter(playlist_tracks::playlist_id.eq(key))
      .order(playlist_tracks::position)
      .select(TrackRow::as_select())
      .load(conn)?;
    let tracks = tracks_from(conn, rows)?;
    Ok(PlaylistDetail { playlist, tracks })
  }
}

impl SettingsStore for SqliteStore {
  /// Falls back to the defaults when nothing was saved yet or the row cannot be read.
  fn playback_settings(&self) -> PlaybackSettings {
    let row = playback_settings::table
      .find(SETTINGS_ID)
      .select(SettingsRow::as_select())
      .first(&mut *self.conn())
      .ok();
    PlaybackSettings {
      volume: row
        .as_ref()
        .and_then(|row| u8::try_from(row.volume).ok())
        .and_then(|level| Volume::new(level).ok())
        .unwrap_or(Volume::DEFAULT),
      shuffle: row.as_ref().is_some_and(|row| row.shuffle),
      repeat: row
        .as_ref()
        .and_then(|row| parse_repeat(&row.repeat))
        .unwrap_or_default(),
      fades: row.as_ref().is_none_or(|row| row.fades),
      crossfade: row
        .as_ref()
        .and_then(|row| u8::try_from(row.crossfade_seconds).ok())
        .and_then(|seconds| Crossfade::new(seconds).ok())
        .unwrap_or_default(),
      discord_status: row.as_ref().is_none_or(|row| row.discord_status),
    }
  }

  fn save_playback_settings(&self, settings: PlaybackSettings) -> AppResult<()> {
    let row = SettingsRow {
      id: SETTINGS_ID,
      volume: settings.volume.percent().into(),
      shuffle: settings.shuffle,
      repeat: repeat_text(settings.repeat).to_owned(),
      fades: settings.fades,
      crossfade_seconds: settings.crossfade.seconds().into(),
      discord_status: settings.discord_status,
    };
    diesel::insert_into(playback_settings::table)
      .values(&row)
      .on_conflict(playback_settings::id)
      .do_update()
      .set(&row)
      .execute(&mut *self.conn())?;
    Ok(())
  }

  fn player_session(&self) -> Option<PlayerSession> {
    let (queue, position_ms): (String, i32) = player_session::table
      .find(SETTINGS_ID)
      .select((player_session::queue, player_session::position_ms))
      .first(&mut *self.conn())
      .ok()?;
    // a queue saved by another version that this one cannot read is left behind like none
    let queue: QueueSnapshot<TrackId> = serde_json::from_str(&queue).ok()?;
    Some(PlayerSession {
      queue,
      position_ms: stored(position_ms).ok()?,
    })
  }

  fn save_player_session(&self, session: &PlayerSession) -> AppResult<()> {
    let queue = serde_json::to_string(&session.queue).map_err(|error| AppError::Storage(error.to_string()))?;
    let row = (
      player_session::id.eq(SETTINGS_ID),
      player_session::queue.eq(&queue),
      player_session::position_ms.eq(column(session.position_ms)?),
    );
    diesel::insert_into(player_session::table)
      .values(row)
      .on_conflict(player_session::id)
      .do_update()
      .set(row)
      .execute(&mut *self.conn())?;
    Ok(())
  }

  fn save_player_position(&self, position_ms: u32) -> AppResult<()> {
    diesel::update(player_session::table.find(SETTINGS_ID))
      .set(player_session::position_ms.eq(column(position_ms)?))
      .execute(&mut *self.conn())?;
    Ok(())
  }
}

#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = download_settings, check_for_backend(Sqlite), treat_none_as_null = true)]
struct DownloadSettingsRow {
  id: i32,
  folder: Option<String>,
  tools_checked_at: Option<i64>,
}

impl DownloadSettingsStore for SqliteStore {
  fn download_settings(&self) -> DownloadSettings {
    download_settings::table
      .find(SETTINGS_ID)
      .select(DownloadSettingsRow::as_select())
      .first(&mut *self.conn())
      .map(|row| DownloadSettings {
        folder: row.folder,
        tools_checked_at: row.tools_checked_at,
      })
      .unwrap_or_default()
  }

  fn save_download_settings(&self, settings: &DownloadSettings) -> AppResult<()> {
    let row = DownloadSettingsRow {
      id: SETTINGS_ID,
      folder: settings.folder.clone(),
      tools_checked_at: settings.tools_checked_at,
    };
    diesel::insert_into(download_settings::table)
      .values(&row)
      .on_conflict(download_settings::id)
      .do_update()
      .set(&row)
      .execute(&mut *self.conn())?;
    Ok(())
  }
}

#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = window_settings, check_for_backend(Sqlite))]
struct WindowSettingsRow {
  id: i32,
  close_to_tray: bool,
  start_hidden: bool,
}

impl WindowSettingsStore for SqliteStore {
  /// Falls back to the defaults when nothing was saved yet or the row cannot be read.
  fn window_settings(&self) -> WindowSettings {
    window_settings::table
      .find(SETTINGS_ID)
      .select(WindowSettingsRow::as_select())
      .first(&mut *self.conn())
      .map(|row| WindowSettings {
        close_to_tray: row.close_to_tray,
        start_hidden: row.start_hidden,
      })
      .unwrap_or_default()
  }

  fn save_window_settings(&self, settings: WindowSettings) -> AppResult<()> {
    let row = WindowSettingsRow {
      id: SETTINGS_ID,
      close_to_tray: settings.close_to_tray,
      start_hidden: settings.start_hidden,
    };
    diesel::insert_into(window_settings::table)
      .values(&row)
      .on_conflict(window_settings::id)
      .do_update()
      .set(&row)
      .execute(&mut *self.conn())?;
    Ok(())
  }
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = downloads)]
struct DownloadRow {
  id: i32,
  request: String,
  folder: String,
  file_name: Option<String>,
  state: String,
  track_id: Option<i32>,
  error: Option<String>,
}

/// Values of `downloads.state`.
const DOWNLOAD_RUNNING: &str = "running";
const DOWNLOAD_DONE: &str = "done";
const DOWNLOAD_FAILED: &str = "failed";

impl DownloadRow {
  fn saved(self) -> AppResult<SavedDownload> {
    let request = serde_json::from_str(&self.request).map_err(|error| AppError::Storage(error.to_string()))?;
    let outcome = match (self.state.as_str(), self.track_id, self.error) {
      (DOWNLOAD_DONE, Some(track), _) => Some(DownloadOutcome::Done(TrackId(stored(track)?))),
      (DOWNLOAD_FAILED, _, Some(error)) => Some(DownloadOutcome::Failed(error)),
      _ => None,
    };
    Ok(SavedDownload {
      id: DownloadId(stored(self.id)?),
      request,
      folder: self.folder.into(),
      file_name: self.file_name,
      outcome,
    })
  }
}

impl DownloadStore for SqliteStore {
  fn saved_downloads(&self) -> AppResult<Vec<SavedDownload>> {
    let rows = downloads::table
      .order(downloads::id)
      .select(DownloadRow::as_select())
      .load(&mut *self.conn())?;
    // a row an older version wrote in a shape this one no longer reads is left out rather than hiding the others
    Ok(
      rows
        .into_iter()
        .filter_map(|row| {
          row
            .saved()
            .inspect_err(|error| eprintln!("skipping a saved download: {error}"))
            .ok()
        })
        .collect(),
    )
  }

  fn add_download(&self, request: &DownloadRequest, folder: &Path) -> AppResult<DownloadId> {
    let request = serde_json::to_string(request).map_err(|error| AppError::Storage(error.to_string()))?;
    let id: i32 = diesel::insert_into(downloads::table)
      .values((
        downloads::request.eq(request),
        downloads::folder.eq(folder.to_string_lossy()),
        downloads::state.eq(DOWNLOAD_RUNNING),
      ))
      .returning(downloads::id)
      .get_result(&mut *self.conn())?;
    Ok(DownloadId(stored(id)?))
  }

  fn set_download_file(&self, id: DownloadId, file_name: &str) -> AppResult<()> {
    diesel::update(downloads::table.find(column(id.0)?))
      .set(downloads::file_name.eq(file_name))
      .execute(&mut *self.conn())?;
    Ok(())
  }

  fn end_download(&self, id: DownloadId, outcome: &DownloadOutcome) -> AppResult<()> {
    let (state, track_id, error) = match outcome {
      DownloadOutcome::Done(track) => (DOWNLOAD_DONE, Some(column(track.0)?), None),
      DownloadOutcome::Failed(error) => (DOWNLOAD_FAILED, None, Some(error.as_str())),
    };
    diesel::update(downloads::table.find(column(id.0)?))
      .set((
        downloads::state.eq(state),
        downloads::track_id.eq(track_id),
        downloads::error.eq(error),
      ))
      .execute(&mut *self.conn())?;
    Ok(())
  }

  fn remove_download(&self, id: DownloadId) -> AppResult<()> {
    diesel::delete(downloads::table.find(column(id.0)?)).execute(&mut *self.conn())?;
    Ok(())
  }
}

#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = lyrics)]
#[diesel(treat_none_as_null = true)]
struct LyricsRow {
  track_id: i32,
  kind: String,
  text: Option<String>,
  lrclib_id: Option<i32>,
  length_ms: i32,
  checked_at: i64,
}

/// Values of `lyrics.kind`.
const LYRICS_UNWRITTEN: &str = "unwritten";
const LYRICS_WRITTEN: &str = "written";
const LYRICS_INSTRUMENTAL: &str = "instrumental";
const LYRICS_MISSING: &str = "missing";

/// The LRCLIB record lyrics came from, kept in `lyrics.lrclib_id`; none for lyrics of the file itself.
fn lrclib_id(origin: LyricsOrigin) -> AppResult<Option<i32>> {
  match origin {
    LyricsOrigin::File => Ok(None),
    LyricsOrigin::Lrclib { id } => Ok(Some(column(id)?)),
  }
}

fn lyrics_origin(lrclib_id: Option<i32>) -> AppResult<LyricsOrigin> {
  Ok(match lrclib_id {
    Some(id) => LyricsOrigin::Lrclib { id: stored(id)? },
    None => LyricsOrigin::File,
  })
}

impl LyricsRow {
  fn new(track_id: i32, record: &LyricsRecord) -> AppResult<Self> {
    let (kind, text, lrclib_id) = match &record.state {
      LyricsState::Unwritten { text, origin } => (LYRICS_UNWRITTEN, Some(text.clone()), lrclib_id(*origin)?),
      LyricsState::Written { origin } => (LYRICS_WRITTEN, None, lrclib_id(*origin)?),
      LyricsState::Instrumental => (LYRICS_INSTRUMENTAL, None, None),
      LyricsState::Missing => (LYRICS_MISSING, None, None),
    };
    Ok(Self {
      track_id,
      kind: kind.to_owned(),
      text,
      lrclib_id,
      length_ms: column(record.length_ms)?,
      checked_at: record.checked_at,
    })
  }

  fn record(self) -> AppResult<LyricsRecord> {
    let state = match (self.kind.as_str(), self.text) {
      (LYRICS_UNWRITTEN, Some(text)) => LyricsState::Unwritten {
        text,
        origin: lyrics_origin(self.lrclib_id)?,
      },
      (LYRICS_WRITTEN, _) => LyricsState::Written {
        origin: lyrics_origin(self.lrclib_id)?,
      },
      (LYRICS_INSTRUMENTAL, _) => LyricsState::Instrumental,
      _ => LyricsState::Missing,
    };
    Ok(LyricsRecord {
      state,
      length_ms: stored(self.length_ms)?,
      checked_at: self.checked_at,
    })
  }
}

impl LyricsStore for SqliteStore {
  fn lyrics_record(&self, track: TrackId) -> AppResult<Option<LyricsRecord>> {
    lyrics::table
      .find(column(track.0)?)
      .select(LyricsRow::as_select())
      .first(&mut *self.conn())
      .optional()?
      .map(LyricsRow::record)
      .transpose()
  }

  fn save_lyrics_record(&self, track: TrackId, record: &LyricsRecord) -> AppResult<()> {
    let row = LyricsRow::new(column(track.0)?, record)?;
    diesel::insert_into(lyrics::table)
      .values(&row)
      .on_conflict(lyrics::track_id)
      .do_update()
      .set(&row)
      .execute(&mut *self.conn())?;
    Ok(())
  }

  fn unwritten_lyrics(&self) -> AppResult<Vec<UnwrittenLyrics>> {
    let rows: Vec<(i32, String, Option<String>)> = lyrics::table
      .inner_join(tracks::table)
      .filter(lyrics::kind.eq(LYRICS_UNWRITTEN))
      .filter(present())
      .select((lyrics::track_id, tracks::path, lyrics::text))
      .load(&mut *self.conn())?;
    rows
      .into_iter()
      .filter_map(|(id, path, text)| text.map(|text| (id, path, text)))
      .map(|(id, path, text)| {
        Ok(UnwrittenLyrics {
          track: TrackId(stored(id)?),
          path,
          text,
        })
      })
      .collect()
  }

  fn mark_lyrics_written(&self, track: TrackId) -> AppResult<()> {
    diesel::update(lyrics::table.find(column(track.0)?))
      // the file holds the text now, so only where it came from is kept
      .set((lyrics::kind.eq(LYRICS_WRITTEN), lyrics::text.eq(None::<String>)))
      .execute(&mut *self.conn())?;
    Ok(())
  }

  fn forget_lyrics(&self, track: TrackId) -> AppResult<()> {
    diesel::delete(lyrics::table.find(column(track.0)?)).execute(&mut *self.conn())?;
    Ok(())
  }
}

#[derive(Insertable)]
#[diesel(table_name = plays)]
struct PlayRow {
  track_id: i32,
  context_kind: Option<&'static str>,
  context_id: Option<i32>,
  started_at: i64,
  heard_ms: i32,
  counted: bool,
}

impl PlayRow {
  fn new(listen: &Listen) -> AppResult<Self> {
    let context = listen.context();
    Ok(Self {
      track_id: column(listen.track().0)?,
      context_kind: context.map(context_kind),
      context_id: column_opt(context.and_then(context_id))?,
      started_at: listen.started_at(),
      heard_ms: i32::try_from(listen.heard().as_millis()).unwrap_or(i32::MAX),
      counted: listen.counts(),
    })
  }
}

impl PlayHistory for SqliteStore {
  fn add_listen(&self, listen: &Listen) -> AppResult<PlayId> {
    let id: i32 = diesel::insert_into(plays::table)
      .values(PlayRow::new(listen)?)
      .returning(plays::id)
      .get_result(&mut *self.conn())?;
    Ok(PlayId(stored(id)?))
  }

  fn update_listen(&self, id: PlayId, listen: &Listen) -> AppResult<()> {
    let row = PlayRow::new(listen)?;
    diesel::update(plays::table.find(column(id.0)?))
      .set((plays::heard_ms.eq(row.heard_ms), plays::counted.eq(row.counted)))
      .execute(&mut *self.conn())?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::artists::Artists;
  use crate::domain::playback::PlaybackContext;

  fn scanned(path: &str, title: &str) -> ScannedTrack {
    ScannedTrack {
      path: path.into(),
      title: title.into(),
      artists: Some(Artists::new(["Artist"]).unwrap()),
      album: Some("Album".into()),
      album_artists: Some(Artists::new(["Artist"]).unwrap()),
      year: None,
      release_type: None,
      track_no: None,
      disc_no: None,
      genres: Vec::new(),
      duration_ms: 1000,
      modified: 1,
      cover: None,
    }
  }

  #[test]
  fn playlists_keep_songs_whose_folder_left_and_get_them_back() {
    let store = SqliteStore::in_memory().unwrap();
    let folder = FolderPath::new(if cfg!(windows) { r"C:\Music" } else { "/music" }).unwrap();
    let with_folder = LibraryFolders::restore(vec![folder.clone()]);
    store.save_folders(&with_folder).unwrap();
    let held = folder.as_path().join("held.mp3").to_string_lossy().into_owned();
    let loose = folder.as_path().join("loose.mp3").to_string_lossy().into_owned();
    store
      .index_tracks(&folder, &[scanned(&held, "Held"), scanned(&loose, "Loose")])
      .unwrap();
    let id = store.track_at(&held).unwrap();
    let name = PlaylistName::new("Mix").unwrap();
    let playlist_id = store.create_playlist(&name, None).unwrap();
    let mut playlist = store.load_playlist(playlist_id).unwrap();
    playlist.add(&[id]);
    store.save_playlist(&playlist).unwrap();

    store.save_folders(&LibraryFolders::default()).unwrap();
    assert!(store.tracks().unwrap().is_empty());
    assert!(store.albums().unwrap().is_empty());
    let tracks = store.playlist(playlist_id).unwrap().tracks;
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].presence, Presence::Missing);
    assert!(store.track_at(&loose).is_err());

    store.save_folders(&with_folder).unwrap();
    store.index_tracks(&folder, &[scanned(&held, "Held")]).unwrap();
    let tracks = store.playlist(playlist_id).unwrap().tracks;
    assert_eq!((tracks[0].id, tracks[0].presence), (id, Presence::Present));
  }

  #[test]
  fn window_settings_round_trip() {
    let store = SqliteStore::in_memory().unwrap();
    assert_eq!(store.window_settings(), WindowSettings::default());
    let saved = WindowSettings {
      close_to_tray: false,
      start_hidden: true,
    };
    store.save_window_settings(saved).unwrap();
    store.save_window_settings(saved).unwrap();
    assert_eq!(store.window_settings(), saved);
  }

  #[test]
  fn pins_keep_their_order_and_leave_with_their_playlist() {
    let store = SqliteStore::in_memory().unwrap();
    let name = |text: &str| PlaylistName::new(text).unwrap();
    let first = store.create_playlist(&name("First"), None).unwrap();
    let second = store.create_playlist(&name("Second"), None).unwrap();
    store
      .save_pins(PinKind::Playlist, &Pins::new([second.0, first.0]))
      .unwrap();
    assert_eq!(store.load_pins(PinKind::Playlist).unwrap().ids(), [second.0, first.0]);
    let pin_of = |id: PlaylistId| store.playlists().unwrap().into_iter().find(|p| p.id == id).unwrap().pin;
    assert_eq!((pin_of(second), pin_of(first)), (Some(0), Some(1)));

    store.delete_playlist(second).unwrap();
    assert_eq!(store.load_pins(PinKind::Playlist).unwrap().ids(), [first.0]);
    assert!(store.load_pins(PinKind::Album).unwrap().ids().is_empty());
  }

  #[test]
  fn guests_are_credited_and_appear_on_the_albums_of_others() {
    let store = SqliteStore::in_memory().unwrap();
    let folder = FolderPath::new(if cfg!(windows) { r"C:\Music" } else { "/music" }).unwrap();
    store
      .save_folders(&LibraryFolders::restore(vec![folder.clone()]))
      .unwrap();
    let path = folder.as_path().join("duet.mp3").to_string_lossy().into_owned();
    let duet = ScannedTrack {
      artists: Some(Artists::new(["Host", "Guest"]).unwrap()),
      album_artists: Some(Artists::new(["Host"]).unwrap()),
      ..scanned(&path, "Duet")
    };
    store.index_tracks(&folder, &[duet]).unwrap();

    let track = store.tracks().unwrap().remove(0);
    let names: Vec<&str> = track.artists.iter().map(|artist| artist.name.as_str()).collect();
    assert_eq!(names, ["Host", "Guest"]);
    let guest = store.artist(track.artists[1].id).unwrap();
    assert!(guest.albums.is_empty());
    assert_eq!(guest.appears_on.len(), 1);
    assert_eq!(guest.tracks.len(), 1);
    assert_eq!(guest.artist.track_count, 1);

    let solo = ScannedTrack {
      artists: Some(Artists::new(["Host"]).unwrap()),
      album_artists: Some(Artists::new(["Host"]).unwrap()),
      ..scanned(&path, "Duet")
    };
    store.retag_tracks(&[solo]).unwrap();
    assert_eq!(store.artists().unwrap().len(), 1);
  }

  #[test]
  fn download_settings_round_trip() {
    let store = SqliteStore::in_memory().unwrap();
    assert!(store.download_settings().folder.is_none());
    let saved = DownloadSettings {
      folder: Some("C:\\Music".into()),
      tools_checked_at: Some(42),
    };
    store.save_download_settings(&saved).unwrap();
    store.save_download_settings(&saved).unwrap();
    let loaded = store.download_settings();
    assert_eq!(loaded.folder, saved.folder);
    assert_eq!(loaded.tools_checked_at, Some(42));
  }

  #[test]
  fn home_shows_what_was_played_and_added_last() {
    use std::time::Duration;

    use crate::application::home;

    let store = SqliteStore::in_memory().unwrap();
    let folder = FolderPath::new(if cfg!(windows) { r"C:\Music" } else { "/music" }).unwrap();
    store
      .save_folders(&LibraryFolders::restore(vec![folder.clone()]))
      .unwrap();
    let path = |name: &str| folder.as_path().join(name).to_string_lossy().into_owned();
    let (one, two) = (path("one.mp3"), path("two.mp3"));
    store
      .index_tracks(
        &folder,
        &[
          scanned(&one, "One"),
          ScannedTrack {
            modified: 2,
            ..scanned(&two, "Two")
          },
        ],
      )
      .unwrap();
    let (one, two) = (store.track_at(&one).unwrap(), store.track_at(&two).unwrap());
    let playlist = store.create_playlist(&PlaylistName::new("Mix").unwrap(), None).unwrap();
    let length = Duration::from_secs(200);
    let heard = |track, context, started_at, seconds| {
      let mut listen = Listen::new(track, context, length, started_at);
      listen.hear(Duration::from_secs(seconds));
      store.add_listen(&listen).unwrap()
    };
    // the first song played on from the playlist until it counts, then a skipped moment of the second one
    let long = heard(one, Some(PlaybackContext::Playlist { id: playlist }), 100, 20);
    let mut longer = Listen::new(one, Some(PlaybackContext::Playlist { id: playlist }), length, 100);
    longer.hear(Duration::from_secs(120));
    store.update_listen(long, &longer).unwrap();
    heard(two, Some(PlaybackContext::Songs), 200, 5);

    let recent = store.scores().unwrap();
    assert_eq!(recent.tracks, vec![(one, 100)]);
    assert_eq!(recent.playlists, vec![(playlist, 100)]);

    // the song lends its play to its album and artist, while the playlist counts only what was played from it
    let page = home::home(&store).unwrap();
    let kinds: Vec<&str> = page
      .recent
      .iter()
      .map(|entry| match entry {
        HomeEntry::Track { .. } => "track",
        HomeEntry::Album { .. } => "album",
        HomeEntry::Artist { .. } => "artist",
        HomeEntry::Playlist { .. } => "playlist",
      })
      .collect();
    assert_eq!(kinds, ["track", "album", "artist", "playlist"]);
    let ids = |tracks: &[Track]| tracks.iter().map(|track| track.id).collect::<Vec<_>>();
    assert_eq!(ids(&home::recent_songs(&store).unwrap()), [one]);
    assert_eq!(ids(&page.added), [two, one]);
  }

  #[test]
  fn player_session_round_trip() {
    let store = SqliteStore::in_memory().unwrap();
    assert_eq!(store.player_session(), None);
    let session = PlayerSession {
      queue: QueueSnapshot {
        context: Some(PlaybackContext::Songs),
        items: vec![TrackId(3), TrackId(1), TrackId(2)],
        unshuffled: Some(vec![1, 2, 0]),
        current: Some(1),
      },
      position_ms: 61_000,
    };
    store.save_player_session(&session).unwrap();
    store.save_player_position(75_500).unwrap();
    assert_eq!(
      store.player_session(),
      Some(PlayerSession {
        position_ms: 75_500,
        ..session
      })
    );
  }

  #[test]
  fn downloads_are_kept_until_removed() {
    use crate::domain::download::AudioQuality;
    let store = SqliteStore::in_memory().unwrap();
    let request = DownloadRequest {
      video_url: "https://www.youtube.com/watch?v=abc".into(),
      cover: None,
      title: "A Lot".into(),
      artists: vec!["21 Savage".into()],
      album: None,
      year: Some(2018),
      quality: AudioQuality::Original,
      folder: None,
      playlist_ids: vec![PlaylistId(4)],
    };
    let running = store.add_download(&request, Path::new("C:/Music")).unwrap();
    let failed = store.add_download(&request, Path::new("C:/Music")).unwrap();
    store.set_download_file(running, "21 Savage - A Lot.m4a").unwrap();
    store
      .end_download(failed, &DownloadOutcome::Failed("no network".into()))
      .unwrap();

    let saved = store.saved_downloads().unwrap();
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[0].id, running);
    assert_eq!(saved[0].file_name.as_deref(), Some("21 Savage - A Lot.m4a"));
    assert!(saved[0].outcome.is_none());
    assert_eq!(saved[0].request.playlist_ids, [PlaylistId(4)]);
    assert!(matches!(&saved[1].outcome, Some(DownloadOutcome::Failed(error)) if error == "no network"));

    store.remove_download(failed).unwrap();
    assert_eq!(store.saved_downloads().unwrap().len(), 1);
  }

  #[test]
  fn playback_settings_round_trip() {
    let store = SqliteStore::in_memory().unwrap();
    let saved = PlaybackSettings {
      volume: Volume::new(35).unwrap(),
      shuffle: true,
      repeat: RepeatMode::One,
      fades: false,
      crossfade: Crossfade::new(6).unwrap(),
      discord_status: false,
    };
    store.save_playback_settings(saved).unwrap();
    let loaded = store.playback_settings();
    assert_eq!(loaded.volume, saved.volume);
    assert!(loaded.shuffle);
    assert_eq!(loaded.repeat, RepeatMode::One);
    assert!(!loaded.fades);
    assert_eq!(loaded.crossfade, saved.crossfade);
    assert!(!loaded.discord_status);
  }

  #[test]
  fn file_database_reopens_without_migrating_again() {
    let path = std::env::temp_dir().join(format!("aio-test-{}.db", std::process::id()));
    let saved = PlaybackSettings {
      volume: Volume::new(12).unwrap(),
      shuffle: false,
      repeat: RepeatMode::All,
      fades: true,
      crossfade: Crossfade::default(),
      discord_status: true,
    };
    SqliteStore::open(&path).unwrap().save_playback_settings(saved).unwrap();
    let reopened = SqliteStore::open(&path).unwrap().playback_settings();
    for suffix in ["", "-wal", "-shm"] {
      let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
    assert_eq!(reopened.volume, saved.volume);
    assert_eq!(reopened.repeat, RepeatMode::All);
  }

  #[test]
  fn every_repeat_mode_has_a_stored_spelling() {
    for mode in REPEAT_MODES {
      assert_eq!(parse_repeat(repeat_text(mode)), Some(mode));
    }
  }

  #[test]
  fn genres_are_counted_the_most_common_first() {
    let store = SqliteStore::in_memory().unwrap();
    let folder = FolderPath::new(if cfg!(windows) { r"C:\Music" } else { "/music" }).unwrap();
    store
      .save_folders(&LibraryFolders::restore(vec![folder.clone()]))
      .unwrap();
    let song = |file: &str, genre: Option<&str>| ScannedTrack {
      genres: genre.into_iter().map(Into::into).collect(),
      ..scanned(&folder.as_path().join(file).to_string_lossy(), file)
    };
    let songs = [
      song("a.mp3", Some("Rock")),
      song("b.mp3", Some("Pop")),
      song("c.mp3", Some("Rock")),
      song("d.mp3", None),
    ];
    store.index_tracks(&folder, &songs).unwrap();

    let genres: Vec<(String, u32)> = store
      .genres()
      .unwrap()
      .into_iter()
      .map(|genre| (genre.name, genre.track_count))
      .collect();
    assert_eq!(genres, [("Rock".into(), 2), ("Pop".into(), 1)]);
  }

  #[test]
  fn songs_without_an_album_are_on_none_and_show_their_own_cover() {
    let store = SqliteStore::in_memory().unwrap();
    let folder = FolderPath::new(if cfg!(windows) { r"C:\Music" } else { "/music" }).unwrap();
    store
      .save_folders(&LibraryFolders::restore(vec![folder.clone()]))
      .unwrap();
    let path = |file: &str| folder.as_path().join(file).to_string_lossy().into_owned();
    let loose = |file: &str, cover: &str| ScannedTrack {
      album: None,
      cover: Some(cover.into()),
      ..scanned(&path(file), file)
    };
    let songs = [
      loose("a.mp3", "a.jpg"),
      loose("b.mp3", "b.jpg"),
      ScannedTrack {
        cover: Some("album.jpg".into()),
        ..scanned(&path("c.mp3"), "c")
      },
    ];
    store.index_tracks(&folder, &songs).unwrap();

    let albums = store.albums().unwrap();
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].track_count, 1);
    let covers: Vec<(Option<AlbumId>, Option<String>)> = store
      .tracks()
      .unwrap()
      .into_iter()
      .map(|track| (track.album_id, track.cover))
      .collect();
    assert!(covers.contains(&(None, Some("a.jpg".into()))));
    assert!(covers.contains(&(None, Some("b.jpg".into()))));
    assert!(covers.contains(&(Some(albums[0].id), Some("album.jpg".into()))));
  }

  #[test]
  fn songs_without_an_artist_are_credited_to_nobody() {
    let store = SqliteStore::in_memory().unwrap();
    let folder = FolderPath::new(if cfg!(windows) { r"C:\Music" } else { "/music" }).unwrap();
    store
      .save_folders(&LibraryFolders::restore(vec![folder.clone()]))
      .unwrap();
    let path = |file: &str| folder.as_path().join(file).to_string_lossy().into_owned();
    let nameless = |file: &str| ScannedTrack {
      artists: None,
      album: Some("Tape".into()),
      album_artists: None,
      ..scanned(&path(file), file)
    };
    let songs = [nameless("a.mp3"), nameless("b.mp3"), scanned(&path("c.mp3"), "c")];
    store.index_tracks(&folder, &songs).unwrap();
    // scanning again finds the same album with no artist instead of making another
    store.retag_tracks(&songs[..1]).unwrap();

    let artists = store.artists().unwrap();
    assert_eq!(artists.len(), 1);
    let albums = store.albums().unwrap();
    let tape = albums.iter().find(|album| album.title == "Tape").unwrap();
    assert_eq!((tape.artists.is_empty(), tape.track_count), (true, 2));
    let tracks = store.tracks().unwrap();
    assert_eq!(tracks.last().map(|track| track.title.as_str()), Some("b.mp3"));
    assert!(
      tracks
        .iter()
        .filter(|track| track.title != "c")
        .all(|track| track.artists.is_empty())
    );
  }

  #[test]
  fn joint_albums_belong_to_each_of_their_artists() {
    let store = SqliteStore::in_memory().unwrap();
    let folder = FolderPath::new(if cfg!(windows) { r"C:\Music" } else { "/music" }).unwrap();
    store
      .save_folders(&LibraryFolders::restore(vec![folder.clone()]))
      .unwrap();
    let path = |file: &str| folder.as_path().join(file).to_string_lossy().into_owned();
    let joint = |file: &str, owners: [&str; 2]| ScannedTrack {
      album: Some("Throne".into()),
      album_artists: Some(Artists::new(owners).unwrap()),
      release_type: Some(ReleaseType::Ep),
      genres: vec!["Rap".into(), "Soul".into()],
      ..scanned(&path(file), file)
    };
    let songs = [
      joint("a.mp3", ["Jay", "Ye"]),
      joint("b.mp3", ["Jay", "Ye"]),
      joint("c.mp3", ["Ye", "Jay"]),
    ];
    store.index_tracks(&folder, &songs).unwrap();

    // the same artists in another order lead another album
    let albums = store.albums().unwrap();
    assert_eq!(albums.len(), 2);
    let throne = albums.iter().find(|album| album.track_count == 2).unwrap();
    let owners: Vec<&str> = throne.artists.iter().map(|artist| artist.name.as_str()).collect();
    assert_eq!(
      (owners, throne.release_type),
      (vec!["Jay", "Ye"], Some(ReleaseType::Ep))
    );
    for owner in &throne.artists {
      assert_eq!(store.artist(owner.id).unwrap().albums.len(), 2);
    }
    let genres: Vec<(String, u32)> = store
      .genres()
      .unwrap()
      .into_iter()
      .map(|genre| (genre.name, genre.track_count))
      .collect();
    assert_eq!(genres, [("Rap".into(), 3), ("Soul".into(), 3)]);
  }

  /// Opens a copy of a real library named by `AIO_DB`, which migrates it, and prints what became of its songs.
  #[test]
  #[ignore]
  fn migrate_library_copy() {
    let path = std::env::var("AIO_DB").expect("AIO_DB names a copy of a library database");
    let store = SqliteStore::open(Path::new(&path)).unwrap();
    let tracks = store.tracks().unwrap();
    let loose = tracks.iter().filter(|track| track.album_id.is_none()).count();
    println!(
      "{} songs, {loose} on no album, {} albums",
      tracks.len(),
      store.albums().unwrap().len()
    );
    assert!(
      store
        .albums()
        .unwrap()
        .iter()
        .all(|album| album.title != "Unknown Album")
    );
    println!("{} playlists", store.playlists().unwrap().len());
  }
}
