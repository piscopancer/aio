use serde::Serialize;
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;

use crate::application::error::AppError;
use crate::application::ports::Notifier;
use crate::application::views::{DownloadView, PlayerState, ToolsView};

/// Full player snapshot, emitted whenever anything except the position changes.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct PlayerStateChanged(pub PlayerState);

/// Position of the loaded track, emitted a few times per second while playing and after seeking.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackProgress {
  pub position_ms: u32,
}

/// A player command was rejected or the sound device failed.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct PlaybackFailed(pub AppError);

/// Tracks, albums or artists changed, so every library view is stale.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct LibraryChanged;

/// Progress of a running library scan.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ScanProgress {
  pub done: u32,
  pub total: u32,
}

/// Every download of the session with its current stage, emitted whenever one of them changes.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct DownloadsChanged(pub Vec<DownloadView>);

/// The download tools were installed, updated or are being fetched.
#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct ToolsChanged(pub ToolsView);

/// Delivers application notifications to the webview as typed Tauri events.
pub struct TauriNotifier(pub AppHandle);

impl TauriNotifier {
  fn emit(&self, event: impl Event + Serialize + Clone) {
    // the webview may be reloading; a missed event is replaced by the next one
    let _ = event.emit(&self.0);
  }
}

impl Notifier for TauriNotifier {
  fn player_changed(&self, state: &PlayerState) {
    self.emit(PlayerStateChanged(state.clone()));
  }

  fn playback_progress(&self, position_ms: u32) {
    self.emit(PlaybackProgress { position_ms });
  }

  fn playback_failed(&self, error: &AppError) {
    eprintln!("playback failed: {error}");
    self.emit(PlaybackFailed(error.clone()));
  }

  fn library_changed(&self) {
    self.emit(LibraryChanged);
  }

  fn scan_progress(&self, done: u32, total: u32) {
    self.emit(ScanProgress { done, total });
  }

  fn downloads_changed(&self, downloads: &[DownloadView]) {
    self.emit(DownloadsChanged(downloads.to_vec()));
  }

  fn tools_changed(&self, tools: &ToolsView) {
    self.emit(ToolsChanged(tools.clone()));
  }
}
