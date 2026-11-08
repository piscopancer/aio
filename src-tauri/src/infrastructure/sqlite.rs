//! SQLite storage through Diesel: every table, column and join below is checked against `schema.rs` at compile time.

use std::collections::HashMap;
use std::hash::Hash;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use diesel::connection::SimpleConnection;
use diesel::dsl::{count, sum};
use diesel::helper_types::{InnerJoin, IsNotNull, IsNull};
use diesel::prelude::*;
use diesel::result::{ConnectionError, Error as DieselError};
use diesel::sqlite::Sqlite;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use super::schema::{
  albums, artists, download_settings, folders, lyrics, playback_settings, playlist_tracks, playlists, track_artists,
  tracks,
};
use crate::application::error::{AppError, AppResult};
use crate::application::ports::*;
use crate::application::views::*;
use crate::domain::ids::{AlbumId, ArtistId, FolderId, PlaylistId, TrackId};
use crate::domain::library::{FolderPath, LibraryFolders};
use crate::domain::pins::{PinKind, Pins};
use crate::domain::playback::{RepeatMode, Volume};
use crate::domain::playlist::{Playlist as PlaylistAggregate, PlaylistName};

/// Files of `src-tauri/migrations`, compiled in and applied at startup.
const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

impl From<DieselError> for AppError {
  fn from(error: DieselError) -> Self {
    match error {
      DieselError::NotFound => Self::NotFound,
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
  album_id: i32,
  #[diesel(select_expression = albums::title)]
  album: String,
  #[diesel(select_expression = albums::cover)]
  cover: Option<String>,
  track_no: Option<i32>,
  disc_no: Option<i32>,
  duration_ms: i32,
  #[diesel(select_expression = tracks::folder_id.is_null(), select_expression_type = IsNull<tracks::folder_id>)]
  missing: bool,
}

/// Tracks whose file is in a library folder; the others are kept only for the playlists that hold them.
fn present() -> IsNotNull<tracks::folder_id> {
  tracks::folder_id.is_not_null()
}

impl TrackRow {
  fn track(self, artists: Vec<ArtistName>) -> AppResult<Track> {
    Ok(Track {
      id: TrackId(stored(self.id)?),
      path: self.path,
      title: self.title,
      artists,
      album_id: AlbumId(stored(self.album_id)?),
      album: self.album,
      cover: self.cover,
      track_no: stored_opt(self.track_no)?,
      disc_no: stored_opt(self.disc_no)?,
      duration_ms: stored(self.duration_ms)?,
      missing: self.missing,
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

/// Tracks with their main artist and album, the source of every `TrackRow`; the main artist only sorts them.
type TrackSource = InnerJoin<InnerJoin<tracks::table, artists::table>, albums::table>;

fn track_source() -> TrackSource {
  tracks::table.inner_join(artists::table).inner_join(albums::table)
}

/// Order of library-wide track lists: by artist, then album, then position on the album.
const LIBRARY_ORDER: (
  artists::name,
  albums::year,
  albums::title,
  tracks::disc_no,
  tracks::track_no,
  tracks::title,
) = (
  artists::name,
  albums::year,
  albums::title,
  tracks::disc_no,
  tracks::track_no,
  tracks::title,
);

#[derive(Queryable, Selectable)]
#[diesel(table_name = albums, check_for_backend(Sqlite))]
struct AlbumRow {
  id: i32,
  title: String,
  artist_id: i32,
  #[diesel(select_expression = artists::name)]
  artist: String,
  year: Option<i32>,
  cover: Option<String>,
  pin: Option<i32>,
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
  artist_id: i32,
  album_id: i32,
  track_no: Option<i32>,
  disc_no: Option<i32>,
  duration_ms: i32,
  modified: i64,
}

#[derive(Queryable, Selectable, Insertable, AsChangeset)]
#[diesel(table_name = playback_settings, check_for_backend(Sqlite))]
struct SettingsRow {
  id: i32,
  volume: i32,
  shuffle: bool,
  repeat: String,
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
      .select((folders::id, folders::path))
      .order(folders::path)
      .into_boxed();
    if let Some(id) = only {
      query = query.filter(folders::id.eq(id));
    }
    let rows: Vec<(i32, String)> = query.load(conn)?;
    let counts: HashMap<i32, i64> = tracks::table
      .filter(present())
      .group_by(tracks::folder_id)
      .select((tracks::folder_id.assume_not_null(), count(tracks::id)))
      .load(conn)?
      .into_iter()
      .collect();
    rows
      .into_iter()
      .map(|(id, path)| {
        Ok(Folder {
          id: FolderId(stored(id)?),
          path,
          track_count: stored(counts.get(&id).copied().unwrap_or_default())?,
        })
      })
      .collect()
  }

  fn load_albums(&self, scope: AlbumScope) -> AppResult<Vec<Album>> {
    let conn = &mut *self.conn();
    let query = albums::table
      .inner_join(artists::table)
      .select(AlbumRow::as_select())
      .into_boxed();
    let query = match scope {
      AlbumScope::All => query.order(albums::title),
      AlbumScope::One(id) => query.filter(albums::id.eq(id)),
      AlbumScope::ByArtist(id) => query
        .filter(albums::artist_id.eq(id))
        .order((albums::year.desc(), albums::title)),
      AlbumScope::AppearsOn(id) => query
        .filter(albums::artist_id.ne(id))
        .filter(
          albums::id.eq_any(
            tracks::table
              .inner_join(track_artists::table)
              .filter(track_artists::artist_id.eq(id))
              .filter(present())
              .select(tracks::album_id),
          ),
        )
        .order((albums::year.desc(), albums::title)),
    };
    let rows = query.load(conn)?;
    let totals = totals_by(
      tracks::table
        .filter(present())
        .group_by(tracks::album_id)
        .select((tracks::album_id, count(tracks::id), sum(tracks::duration_ms)))
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
          artist_id: ArtistId(stored(row.artist_id)?),
          artist: row.artist,
          year: stored_opt(row.year)?,
          cover: row.cover,
          track_count: tracks,
          duration_ms,
          pin: stored_opt(row.pin)?,
        })
      })
      .collect()
  }

  /// Artists credited on at least one track, as its main artist or a guest; one who only appears as an album artist
  /// is left out.
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
    let album_counts: HashMap<i32, i64> = albums::table
      .group_by(albums::artist_id)
      .select((albums::artist_id, count(albums::id)))
      .load(conn)?
      .into_iter()
      .collect();
    // the newest album with art stands for the artist, or else the newest one they sing on as a guest
    let covers = first_per_key(
      albums::table
        .filter(albums::cover.is_not_null())
        .order(albums::year.desc())
        .select((albums::artist_id, albums::cover.assume_not_null()))
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
    rows
      .into_iter()
      .filter_map(|(id, name, pin)| {
        let track_count = *track_counts.get(&id)?;
        Some((id, name, pin, track_count))
      })
      .map(|(id, name, pin, track_count)| {
        Ok(Artist {
          id: ArtistId(stored(id)?),
          name,
          cover: covers.get(&id).or_else(|| guest_covers.get(&id)).cloned(),
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
    // the art of the first track that has any stands for the playlist
    let covers = first_per_key(
      playlist_tracks::table
        .inner_join(tracks::table.inner_join(albums::table))
        .filter(albums::cover.is_not_null())
        .order((playlist_tracks::playlist_id, playlist_tracks::position))
        .select((playlist_tracks::playlist_id, albums::cover.assume_not_null()))
        .load::<(i32, String)>(conn)?,
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

fn single<T>(rows: Vec<T>) -> AppResult<T> {
  rows.into_iter().next().ok_or(AppError::NotFound)
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

/// Finds or creates the album of a scanned track and settles its year and cover by `art`.
fn upsert_album(conn: &mut SqliteConnection, track: &ScannedTrack, artist_id: i32, art: AlbumArt) -> AppResult<i32> {
  let year = column_opt(track.year)?;
  let existing = albums::table
    .filter(albums::title.eq(&track.album))
    .filter(albums::artist_id.eq(artist_id))
    .select(albums::id)
    .first::<i32>(conn)
    .optional()?;
  let Some(id) = existing else {
    let id = diesel::insert_into(albums::table)
      .values((
        albums::title.eq(&track.album),
        albums::artist_id.eq(artist_id),
        albums::year.eq(year),
        albums::cover.eq(&track.cover),
      ))
      .returning(albums::id)
      .get_result(conn)?;
    return Ok(id);
  };
  if art == AlbumArt::Replace {
    diesel::update(albums::table.find(id))
      .set((albums::year.eq(year), albums::cover.eq(&track.cover)))
      .execute(conn)?;
    return Ok(id);
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

/// Ids of the credited artists, in order, and of the album a scanned track belongs to, creating them when new.
fn track_owners(conn: &mut SqliteConnection, track: &ScannedTrack, art: AlbumArt) -> AppResult<(Vec<i32>, i32)> {
  let artist_ids = track
    .artists
    .names()
    .iter()
    .map(|name| upsert_artist(conn, name))
    .collect::<AppResult<Vec<_>>>()?;
  let album_artist_id = upsert_artist(conn, &track.album_artist)?;
  Ok((artist_ids, upsert_album(conn, track, album_artist_id, art)?))
}

/// The first of the credited artists, whom the track row keeps for sorting.
fn main_artist(artist_ids: &[i32]) -> AppResult<i32> {
  artist_ids
    .first()
    .copied()
    .ok_or_else(|| corrupt("track without artists"))
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
  diesel::insert_into(track_artists::table).values(&rows).execute(conn)?;
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
  diesel::delete(albums::table.filter(albums::id.ne_all(tracks::table.select(tracks::album_id)))).execute(conn)?;
  diesel::delete(
    artists::table
      .filter(artists::id.ne_all(track_artists::table.select(track_artists::artist_id)))
      .filter(artists::id.ne_all(albums::table.select(albums::artist_id))),
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
      diesel::delete(folders::table.filter(folders::path.ne_all(&kept))).execute(conn)?;
      for path in &kept {
        diesel::insert_or_ignore_into(folders::table)
          .values(folders::path.eq(path))
          .execute(conn)?;
      }
      remove_orphans(conn)
    })
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
      .first(conn)?;
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
        return Err(AppError::NotFound);
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
      0 => Err(AppError::NotFound),
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
        0 => Err(AppError::NotFound),
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
        .first(conn)?;
      for track in scanned {
        let (artist_ids, album_id) = track_owners(conn, track, AlbumArt::Fill)?;
        let row = NewTrack {
          folder_id: Some(folder_id),
          path: &track.path,
          title: &track.title,
          artist_id: main_artist(&artist_ids)?,
          album_id,
          track_no: column_opt(track.track_no)?,
          disc_no: column_opt(track.disc_no)?,
          duration_ms: column(track.duration_ms)?,
          modified: track.modified,
        };
        let track_id: i32 = diesel::insert_into(tracks::table)
          .values(&row)
          .on_conflict(tracks::path)
          .do_update()
          .set(&row)
          .returning(tracks::id)
          .get_result(conn)?;
        set_credits(conn, track_id, &artist_ids)?;
      }
      remove_orphans(conn)
    })
  }

  fn track_at(&self, path: &str) -> AppResult<TrackId> {
    let id: i32 = tracks::table
      .filter(tracks::path.eq(path))
      .select(tracks::id)
      .first(&mut *self.conn())?;
    Ok(TrackId(stored(id)?))
  }

  fn retag_tracks(&self, scanned: &[ScannedTrack]) -> AppResult<()> {
    self.write(|conn| {
      for track in scanned {
        let (artist_ids, album_id) = track_owners(conn, track, AlbumArt::Replace)?;
        let track_id: i32 = diesel::update(tracks::table.filter(tracks::path.eq(&track.path)))
          .set((
            tracks::title.eq(&track.title),
            tracks::artist_id.eq(main_artist(&artist_ids)?),
            tracks::album_id.eq(album_id),
            tracks::track_no.eq(column_opt(track.track_no)?),
            tracks::disc_no.eq(column_opt(track.disc_no)?),
            tracks::duration_ms.eq(column(track.duration_ms)?),
            tracks::modified.eq(track.modified),
          ))
          .returning(tracks::id)
          .get_result(conn)?;
        set_credits(conn, track_id, &artist_ids)?;
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
    let folder = single(self.folder_views(Some(key))?)?;
    let conn = &mut *self.conn();
    let rows = track_source()
      .filter(tracks::folder_id.eq(key))
      .order(LIBRARY_ORDER)
      .select(TrackRow::as_select())
      .load(conn)?;
    let tracks = tracks_from(conn, rows)?;
    Ok(FolderDetail { folder, tracks })
  }

  fn tracks(&self) -> AppResult<Vec<Track>> {
    let conn = &mut *self.conn();
    let rows = track_source()
      .filter(present())
      .select(TrackRow::as_select())
      .order(LIBRARY_ORDER)
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
    let album = single(self.load_albums(AlbumScope::One(key))?)?;
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

  fn artist(&self, id: ArtistId) -> AppResult<ArtistDetail> {
    let key = column(id.0)?;
    let artist = single(self.load_artists(Some(key))?)?;
    let albums = self.load_albums(AlbumScope::ByArtist(key))?;
    let appears_on = self.load_albums(AlbumScope::AppearsOn(key))?;
    let conn = &mut *self.conn();
    let credited = track_artists::table
      .filter(track_artists::artist_id.eq(key))
      .select(track_artists::track_id);
    let rows = track_source()
      .filter(tracks::id.eq_any(credited))
      .filter(present())
      .order(LIBRARY_ORDER)
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

  fn playlist(&self, id: PlaylistId) -> AppResult<PlaylistDetail> {
    let key = column(id.0)?;
    let playlist = single(self.load_playlists(Some(key))?)?;
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
    }
  }

  fn save_playback_settings(&self, settings: PlaybackSettings) -> AppResult<()> {
    let row = SettingsRow {
      id: SETTINGS_ID,
      volume: settings.volume.percent().into(),
      shuffle: settings.shuffle,
      repeat: repeat_text(settings.repeat).to_owned(),
    };
    diesel::insert_into(playback_settings::table)
      .values(&row)
      .on_conflict(playback_settings::id)
      .do_update()
      .set(&row)
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
#[diesel(table_name = lyrics)]
#[diesel(treat_none_as_null = true)]
struct LyricsRow {
  track_id: i32,
  kind: String,
  text: Option<String>,
  lrclib_id: Option<i32>,
  length_ms: i32,
  in_file: bool,
  checked_at: i64,
}

/// Values of `lyrics.kind`.
const LYRICS_TEXT: &str = "text";
const LYRICS_INSTRUMENTAL: &str = "instrumental";
const LYRICS_MISSING: &str = "missing";

impl LyricsRow {
  fn new(track_id: i32, record: &LyricsRecord) -> AppResult<Self> {
    let (kind, text, lrclib_id) = match &record.find {
      LyricsFind::Text { text, origin } => (
        LYRICS_TEXT,
        Some(text.clone()),
        match origin {
          LyricsOrigin::File => None,
          LyricsOrigin::Lrclib { id } => Some(column(*id)?),
        },
      ),
      LyricsFind::Instrumental => (LYRICS_INSTRUMENTAL, None, None),
      LyricsFind::Missing => (LYRICS_MISSING, None, None),
    };
    Ok(Self {
      track_id,
      kind: kind.to_owned(),
      text,
      lrclib_id,
      length_ms: column(record.length_ms)?,
      in_file: record.in_file,
      checked_at: record.checked_at,
    })
  }

  fn record(self) -> AppResult<LyricsRecord> {
    let find = match (self.kind.as_str(), self.text) {
      (LYRICS_TEXT, Some(text)) => LyricsFind::Text {
        text,
        origin: match self.lrclib_id {
          Some(id) => LyricsOrigin::Lrclib { id: stored(id)? },
          None => LyricsOrigin::File,
        },
      },
      (LYRICS_INSTRUMENTAL, _) => LyricsFind::Instrumental,
      _ => LyricsFind::Missing,
    };
    Ok(LyricsRecord {
      find,
      length_ms: stored(self.length_ms)?,
      in_file: self.in_file,
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
      .filter(lyrics::kind.eq(LYRICS_TEXT))
      .filter(lyrics::in_file.eq(false))
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
      .set(lyrics::in_file.eq(true))
      .execute(&mut *self.conn())?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::domain::artists::Artists;

  fn scanned(path: &str, title: &str) -> ScannedTrack {
    ScannedTrack {
      path: path.into(),
      title: title.into(),
      artists: Artists::new(["Artist"]).unwrap(),
      album: "Album".into(),
      album_artist: "Artist".into(),
      year: None,
      track_no: None,
      disc_no: None,
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
    assert!(tracks[0].missing);
    assert!(store.track_at(&loose).is_err());

    store.save_folders(&with_folder).unwrap();
    store.index_tracks(&folder, &[scanned(&held, "Held")]).unwrap();
    let tracks = store.playlist(playlist_id).unwrap().tracks;
    assert_eq!((tracks[0].id, tracks[0].missing), (id, false));
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
      artists: Artists::new(["Host", "Guest"]).unwrap(),
      album_artist: "Host".into(),
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
      artists: Artists::new(["Host"]).unwrap(),
      album_artist: "Host".into(),
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
  fn playback_settings_round_trip() {
    let store = SqliteStore::in_memory().unwrap();
    let saved = PlaybackSettings {
      volume: Volume::new(35).unwrap(),
      shuffle: true,
      repeat: RepeatMode::One,
    };
    store.save_playback_settings(saved).unwrap();
    let loaded = store.playback_settings();
    assert_eq!(loaded.volume, saved.volume);
    assert!(loaded.shuffle);
    assert_eq!(loaded.repeat, RepeatMode::One);
  }

  #[test]
  fn file_database_reopens_without_migrating_again() {
    let path = std::env::temp_dir().join(format!("aio-test-{}.db", std::process::id()));
    let saved = PlaybackSettings {
      volume: Volume::new(12).unwrap(),
      shuffle: false,
      repeat: RepeatMode::All,
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
}
