use clap::{Parser, ValueEnum};
use serde::Serialize;
use specta::Type;

/// Prepared app state for development, chosen with `pnpm dev --scenario <name>`; it runs on an in-memory database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum Scenario {
  /// Fresh install without folders, tracks or playlists.
  Empty,
  /// About 1500 made-up tracks by 40 artists with a few playlists.
  Library,
  /// The library with an album already playing from the middle of a track, shuffle on and repeat all.
  Playing,
}

#[derive(Debug, Parser)]
#[command(name = "aio", about = "Local music player")]
pub struct Cli {
  /// Start in a prepared development state instead of the real library.
  #[cfg(debug_assertions)]
  #[arg(long, value_enum)]
  scenario: Option<Scenario>,
  /// Set by the entry Windows starts the app with at sign-in, which may keep it in the tray.
  #[arg(long, hide = true)]
  pub autostart: bool,
}

/// The flag the autostart entry passes, as `Cli` parses it.
pub const AUTOSTART_FLAG: &str = "--autostart";

impl Cli {
  /// The requested scenario; release builds have no such flag and always run the real library.
  pub fn scenario(&self) -> Option<Scenario> {
    #[cfg(debug_assertions)]
    return self.scenario;
    #[cfg(not(debug_assertions))]
    return None;
  }
}
