use std::sync::Arc;

use tauri::State;

use super::cli::Scenario;
use crate::application::covers::CoverService;
use crate::application::downloads::{DownloadRequest, DownloadService};
use crate::application::error::{AppError, AppResult};
use crate::application::library::LibraryService;
use crate::application::playback::{PlayerCommand, PlayerHandle};
use crate::application::playlists::PlaylistService;
use crate::application::ports::LibraryQueries;
use crate::application::views::*;
use crate::domain::ids::{AlbumId, ArtistId, DownloadId, FolderId, PlaylistId, TrackId};
use crate::domain::playback::{PlaybackContext, QueueKey, RepeatMode, Volume};
use crate::domain::playlist::AddOutcome;

/// Everything the commands reach, wired once at startup.
pub struct AppState {
  pub library: Arc<LibraryService>,
  pub playlists: Arc<PlaylistService>,
  pub covers: Arc<CoverService>,
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
  let q = queries(&state);
  blocking(move || q.folders()).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_folder(state: Ctx<'_>, id: FolderId) -> AppResult<FolderDetail> {
  let q = queries(&state);
  blocking(move || q.folder(id)).await
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

/// Plays a view's tracks in its order, starting at `start_track_id` or at the first track.
#[tauri::command]
#[specta::specta]
pub async fn play(state: Ctx<'_>, context: PlaybackContext, start_track_id: Option<TrackId>) -> AppResult<()> {
  let (player, q) = (state.player.clone(), queries(&state));
  blocking(move || player.play(q.as_ref(), context, start_track_id)).await
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
pub fn set_repeat(state: Ctx<'_>, repeat: RepeatMode) {
  state.player.send(PlayerCommand::SetRepeat(repeat));
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
