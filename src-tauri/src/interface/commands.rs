use std::sync::Arc;

use tauri::State;

use super::cli::Scenario;
use crate::application::covers::CoverService;
use crate::application::downloads::{DownloadRequest, DownloadService};
use crate::application::error::{AppError, AppResult};
use crate::application::library::LibraryService;
use crate::application::lyrics::LyricsService;
use crate::application::pins::PinService;
use crate::application::playback::{PlayerCommand, PlayerHandle};
use crate::application::playlists::PlaylistService;
use crate::application::ports::LibraryQueries;
use crate::application::tag_search::{TagSearchQuery, TagSearchService};
use crate::application::tags::{TagService, TrackEdit};
use crate::application::views::*;
use crate::domain::ids::{AlbumId, ArtistId, DownloadId, FolderId, PlaylistId, TrackId};
use crate::domain::pins::PinTarget;
use crate::domain::playback::{Crossfade, PlaybackContext, QueueKey, RepeatMode, Volume};
use crate::domain::playlist::AddOutcome;

/// Everything the commands reach, wired once at startup.
pub struct AppState {
  pub library: Arc<LibraryService>,
  pub playlists: Arc<PlaylistService>,
  pub pins: Arc<PinService>,
  pub covers: Arc<CoverService>,
  pub tags: Arc<TagService>,
  pub tag_search: Arc<TagSearchService>,
  pub lyrics: Arc<LyricsService>,
  pub downloads: Arc<DownloadService>,
  pub queries: Arc<dyn LibraryQueries>,
  pub player: Arc<PlayerHandle>,
  pub scenario: Option<Scenario>,
}

type Ctx<'a> = State<'a, AppState>;

/// Runs a blocking use case off the async runtime, so storage and disk work never stall the webview.
async fn blocking<T: Send + 'static>(job: impl FnOnce() -> AppResult<T> + Send + 'static) -> AppResult<T> {
  tauri::async_runtime::spawn_blocking(job)
    .await
    .map_err(|e| AppError::Storage(e.to_string()))?
}

fn queries(state: &AppState) -> Arc<dyn LibraryQueries> {
  state.queries.clone()
}

#[tauri::command]
#[specta::specta]
pub fn get_scenario(state: Ctx<'_>) -> Option<Scenario> {
  state.scenario
}

#[tauri::command]
#[specta::specta]
pub async fn list_folders(state: Ctx<'_>) -> AppResult<Vec<Folder>> {
  let (q, library) = (queries(&state), state.library.clone());
  blocking(move || Ok(library.mark_system(q.folders()?))).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_folder(state: Ctx<'_>, id: FolderId) -> AppResult<FolderDetail> {
  let (q, library) = (queries(&state), state.library.clone());
  blocking(move || {
    let FolderDetail { folder, tracks } = q.folder(id)?;
    let folder = library.mark_system(vec![folder]).remove(0);
    Ok(FolderDetail { folder, tracks })
  })
  .await
}

/// Adds a folder to the library and scans it.
#[tauri::command]
#[specta::specta]
pub async fn add_folder(state: Ctx<'_>, path: String) -> AppResult<()> {
  let library = state.library.clone();
  blocking(move || {
    library.add_folder(&path)?;
    library.scan()
  })
  .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_folder(state: Ctx<'_>, path: String) -> AppResult<()> {
  let library = state.library.clone();
  blocking(move || library.remove_folder(&path)).await
}

/// Hides a folder's songs from the library and the queue, or shows them again.
#[tauri::command]
#[specta::specta]
pub async fn set_folder_hidden(state: Ctx<'_>, path: String, hidden: bool) -> AppResult<()> {
  let (library, player) = (state.library.clone(), state.player.clone());
  blocking(move || {
    library.set_folder_hidden(&path, hidden)?;
    if hidden {
      player.send(PlayerCommand::Prune);
    }
    Ok(())
  })
  .await
}

#[tauri::command]
#[specta::specta]
pub async fn scan_library(state: Ctx<'_>) -> AppResult<()> {
  let library = state.library.clone();
  blocking(move || library.scan()).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_tracks(state: Ctx<'_>) -> AppResult<Vec<Track>> {
  let q = queries(&state);
  blocking(move || q.tracks()).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_albums(state: Ctx<'_>) -> AppResult<Vec<Album>> {
  let q = queries(&state);
  blocking(move || q.albums()).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_album(state: Ctx<'_>, id: AlbumId) -> AppResult<AlbumDetail> {
  let q = queries(&state);
  blocking(move || q.album(id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_artists(state: Ctx<'_>) -> AppResult<Vec<Artist>> {
  let q = queries(&state);
  blocking(move || q.artists()).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_genres(state: Ctx<'_>) -> AppResult<Vec<Genre>> {
  let q = queries(&state);
  blocking(move || q.genres()).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_artist(state: Ctx<'_>, id: ArtistId) -> AppResult<ArtistDetail> {
  let q = queries(&state);
  blocking(move || q.artist(id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_playlists(state: Ctx<'_>) -> AppResult<Vec<Playlist>> {
  let q = queries(&state);
  blocking(move || q.playlists()).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_playlist(state: Ctx<'_>, id: PlaylistId) -> AppResult<PlaylistDetail> {
  let q = queries(&state);
  blocking(move || q.playlist(id)).await
}

/// The playlists that already hold the song, which its add to playlist menu greys out.
#[tauri::command]
#[specta::specta]
pub async fn playlists_with_track(state: Ctx<'_>, track_id: TrackId) -> AppResult<Vec<PlaylistId>> {
  let q = queries(&state);
  blocking(move || q.playlists_with_track(track_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_playlist(state: Ctx<'_>, name: String, cover: Option<String>) -> AppResult<PlaylistId> {
  let playlists = state.playlists.clone();
  blocking(move || playlists.create(&name, cover.as_deref())).await
}

/// Sets the name and the picked cover of a playlist; a null cover lets its songs' art stand in.
#[tauri::command]
#[specta::specta]
pub async fn edit_playlist(state: Ctx<'_>, id: PlaylistId, name: String, cover: Option<String>) -> AppResult<()> {
  let playlists = state.playlists.clone();
  blocking(move || playlists.edit(id, &name, cover)).await
}

/// Stores a square copy of the image file at `file` and returns the copy's path, for a playlist's or a download's cover.
#[tauri::command]
#[specta::specta]
pub async fn import_cover(state: Ctx<'_>, file: String) -> AppResult<String> {
  let covers = state.covers.clone();
  blocking(move || covers.import(&file)).await
}

/// The colour that covers most of the image at `file`, as `#rrggbb`, which pages tint their header with.
#[tauri::command]
#[specta::specta]
pub async fn cover_color(state: Ctx<'_>, file: String) -> AppResult<String> {
  let covers = state.covers.clone();
  blocking(move || covers.color(&file)).await
}

/// A few harmonious colours of the image at `file`, as `#rrggbb`, strongest first, which the lyrics background flows
/// with.
#[tauri::command]
#[specta::specta]
pub async fn cover_palette(state: Ctx<'_>, file: String) -> AppResult<Vec<String>> {
  let covers = state.covers.clone();
  blocking(move || covers.palette(&file)).await
}

/// Variants of a song's tags and covers from online music databases, looked up by the tags the edit form holds now.
#[tauri::command]
#[specta::specta]
pub async fn search_tags(state: Ctx<'_>, id: TrackId, query: TagSearchQuery) -> AppResult<TagSearch> {
  let search = state.tag_search.clone();
  blocking(move || search.search(id, &query)).await
}

/// Downloads a cover the tag search found and stores it, returning its path for the edit form to set.
#[tauri::command]
#[specta::specta]
pub async fn import_found_cover(state: Ctx<'_>, url: String) -> AppResult<String> {
  let search = state.tag_search.clone();
  blocking(move || search.import_cover(&url)).await
}

/// Lyrics of a song, from its file or else looked up on LRCLIB.
#[tauri::command]
#[specta::specta]
pub async fn track_lyrics(state: Ctx<'_>, id: TrackId) -> AppResult<LyricsView> {
  let lyrics = state.lyrics.clone();
  blocking(move || lyrics.lyrics(id)).await
}

/// What LRCLIB has for a song whose file holds other lyrics, if anything.
#[tauri::command]
#[specta::specta]
pub async fn lrclib_lyrics(state: Ctx<'_>, id: TrackId) -> AppResult<Option<LyricsView>> {
  let lyrics = state.lyrics.clone();
  blocking(move || lyrics.lrclib_lyrics(id)).await
}

/// Writes lyrics into the song's file over those it holds.
#[tauri::command]
#[specta::specta]
pub async fn save_lyrics(state: Ctx<'_>, id: TrackId, text: String, origin: LyricsOrigin) -> AppResult<()> {
  let lyrics = state.lyrics.clone();
  blocking(move || lyrics.save(id, &text, origin)).await
}

/// Tags of a song as the edit dialog shows them.
#[tauri::command]
#[specta::specta]
pub async fn track_tags(state: Ctx<'_>, id: TrackId) -> AppResult<TrackTags> {
  let tags = state.tags.clone();
  blocking(move || tags.tags(id)).await
}

/// Rewrites a song's tags; a playing song resumes where it was once its file is written.
#[tauri::command]
#[specta::specta]
pub async fn edit_track(state: Ctx<'_>, id: TrackId, edit: TrackEdit) -> AppResult<()> {
  let tags = state.tags.clone();
  blocking(move || tags.edit(id, edit)).await
}

/// Pins a playlist, album or artist first among the pinned of its list.
#[tauri::command]
#[specta::specta]
pub async fn pin(state: Ctx<'_>, target: PinTarget) -> AppResult<()> {
  let pins = state.pins.clone();
  blocking(move || pins.pin(target)).await
}

#[tauri::command]
#[specta::specta]
pub async fn unpin(state: Ctx<'_>, target: PinTarget) -> AppResult<()> {
  let pins = state.pins.clone();
  blocking(move || pins.unpin(target)).await
}

/// Moves a pinned item to place `to` among the pinned of its list.
#[tauri::command]
#[specta::specta]
pub async fn move_pin(state: Ctx<'_>, target: PinTarget, to: u32) -> AppResult<()> {
  let pins = state.pins.clone();
  blocking(move || pins.move_pin(target, to)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_playlist(state: Ctx<'_>, id: PlaylistId) -> AppResult<()> {
  let playlists = state.playlists.clone();
  blocking(move || playlists.delete(id)).await
}

/// Appends tracks the playlist does not have yet and reports how many were skipped as already present.
#[tauri::command]
#[specta::specta]
pub async fn add_to_playlist(state: Ctx<'_>, id: PlaylistId, track_ids: Vec<TrackId>) -> AppResult<AddOutcome> {
  let playlists = state.playlists.clone();
  blocking(move || playlists.add_tracks(id, &track_ids)).await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_from_playlist(state: Ctx<'_>, id: PlaylistId, track_ids: Vec<TrackId>) -> AppResult<()> {
  let playlists = state.playlists.clone();
  blocking(move || playlists.remove_tracks(id, &track_ids)).await
}

#[tauri::command]
#[specta::specta]
pub async fn move_playlist_track(state: Ctx<'_>, id: PlaylistId, track_id: TrackId, to: u32) -> AppResult<()> {
  let playlists = state.playlists.clone();
  blocking(move || playlists.move_track(id, track_id, to)).await
}

#[tauri::command]
#[specta::specta]
pub fn get_player_state(state: Ctx<'_>) -> PlayerState {
  state.player.state()
}

/// Plays a view's tracks in its order, or in `order` when the view is sorted, starting at `start_track_id` or at the first track.
#[tauri::command]
#[specta::specta]
pub async fn play(
  state: Ctx<'_>,
  context: PlaybackContext,
  start_track_id: Option<TrackId>,
  order: Option<Vec<TrackId>>,
) -> AppResult<()> {
  let (player, q, playlists) = (state.player.clone(), queries(&state), state.playlists.clone());
  blocking(move || {
    player.play(q.as_ref(), context, start_track_id, order.as_deref())?;
    match context {
      PlaybackContext::Playlist { id } => playlists.played(id),
      _ => Ok(()),
    }
  })
  .await
}

#[tauri::command]
#[specta::specta]
pub async fn enqueue_tracks(state: Ctx<'_>, track_ids: Vec<TrackId>) -> AppResult<()> {
  let (player, q) = (state.player.clone(), queries(&state));
  blocking(move || player.enqueue(q.as_ref(), &track_ids)).await
}

#[tauri::command]
#[specta::specta]
pub fn play_queue_entry(state: Ctx<'_>, key: QueueKey) {
  state.player.send(PlayerCommand::Jump(key));
}

#[tauri::command]
#[specta::specta]
pub fn remove_from_queue(state: Ctx<'_>, key: QueueKey) {
  state.player.send(PlayerCommand::Remove(key));
}

#[tauri::command]
#[specta::specta]
pub fn toggle_playback(state: Ctx<'_>) {
  state.player.send(PlayerCommand::Toggle);
}

#[tauri::command]
#[specta::specta]
pub fn next_track(state: Ctx<'_>) {
  state.player.send(PlayerCommand::Next);
}

#[tauri::command]
#[specta::specta]
pub fn previous_track(state: Ctx<'_>) {
  state.player.send(PlayerCommand::Previous);
}

#[tauri::command]
#[specta::specta]
pub fn seek(state: Ctx<'_>, position_ms: u32) {
  state.player.send(PlayerCommand::Seek(position_ms));
}

#[tauri::command]
#[specta::specta]
pub fn set_volume(state: Ctx<'_>, volume: u8) -> AppResult<()> {
  state.player.send(PlayerCommand::SetVolume(Volume::new(volume)?));
  Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn set_shuffle(state: Ctx<'_>, shuffle: bool) {
  state.player.send(PlayerCommand::SetShuffle(shuffle));
}

#[tauri::command]
#[specta::specta]
pub fn set_muted(state: Ctx<'_>, muted: bool) {
  state.player.send(PlayerCommand::SetMuted(muted));
}

#[tauri::command]
#[specta::specta]
pub fn set_repeat(state: Ctx<'_>, repeat: RepeatMode) {
  state.player.send(PlayerCommand::SetRepeat(repeat));
}

/// Turns the short fades as songs start, stop, pause and resume on or off.
#[tauri::command]
#[specta::specta]
pub fn set_fades(state: Ctx<'_>, fades: bool) {
  state.player.send(PlayerCommand::SetFades(fades));
}

/// Sets how many seconds the end of a song and the start of the next play over each other; zero turns it off.
#[tauri::command]
#[specta::specta]
pub fn set_crossfade(state: Ctx<'_>, seconds: u8) -> AppResult<()> {
  state.player.send(PlayerCommand::SetCrossfade(Crossfade::new(seconds)?));
  Ok(())
}

/// Lets Discord show the playing song on the user's profile, or stops it.
#[tauri::command]
#[specta::specta]
pub fn set_discord_status(state: Ctx<'_>, shown: bool) {
  state.player.send(PlayerCommand::SetDiscordStatus(shown));
}

#[tauri::command]
#[specta::specta]
pub fn get_download_settings(state: Ctx<'_>) -> DownloadSettingsView {
  state.downloads.settings()
}

/// Sets the folder downloads go to; null goes back to the system Music folder.
#[tauri::command]
#[specta::specta]
pub async fn set_download_folder(state: Ctx<'_>, folder: Option<String>) -> AppResult<DownloadSettingsView> {
  let downloads = state.downloads.clone();
  blocking(move || downloads.set_folder(folder)).await
}

/// Where files saved into `folder` (or the default folder when null) stand relative to the library.
#[tauri::command]
#[specta::specta]
pub async fn check_download_folder(state: Ctx<'_>, folder: Option<String>) -> AppResult<FolderCheck> {
  let downloads = state.downloads.clone();
  blocking(move || downloads.check_folder(folder)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_download_tools(state: Ctx<'_>) -> AppResult<ToolsView> {
  let downloads = state.downloads.clone();
  blocking(move || Ok(downloads.tools())).await
}

/// Installs missing download tools and checks for updates once a day; progress arrives as events.
#[tauri::command]
#[specta::specta]
pub async fn prepare_downloads(state: Ctx<'_>) -> AppResult<ToolsView> {
  let downloads = state.downloads.clone();
  blocking(move || downloads.prepare()).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_download_tools(state: Ctx<'_>) -> AppResult<ToolsView> {
  let downloads = state.downloads.clone();
  blocking(move || downloads.update_tools()).await
}

/// Resolves a pasted song link to what would be downloaded.
#[tauri::command]
#[specta::specta]
pub async fn preview_download(state: Ctx<'_>, link: String) -> AppResult<DownloadPreview> {
  let downloads = state.downloads.clone();
  blocking(move || downloads.preview(&link)).await
}

/// Songs and videos on YouTube Music matching a typed name.
#[tauri::command]
#[specta::specta]
pub async fn search_downloads(state: Ctx<'_>, query: String) -> AppResult<SongFinds> {
  let downloads = state.downloads.clone();
  blocking(move || downloads.search(&query)).await
}

/// A direct address of a found song's audio, to listen to it before downloading.
#[tauri::command]
#[specta::specta]
pub async fn stream_download(state: Ctx<'_>, link: String) -> AppResult<String> {
  let downloads = state.downloads.clone();
  blocking(move || downloads.stream(&link)).await
}

#[tauri::command]
#[specta::specta]
pub async fn start_download(state: Ctx<'_>, request: DownloadRequest) -> AppResult<DownloadId> {
  let downloads = state.downloads.clone();
  blocking(move || downloads.start(request)).await
}

#[tauri::command]
#[specta::specta]
pub fn list_downloads(state: Ctx<'_>) -> Vec<DownloadView> {
  state.downloads.downloads()
}

#[tauri::command]
#[specta::specta]
pub fn dismiss_download(state: Ctx<'_>, id: DownloadId) {
  state.downloads.dismiss(id);
}

#[tauri::command]
#[specta::specta]
pub fn cancel_download(state: Ctx<'_>, id: DownloadId) {
  state.downloads.cancel(id);
}
