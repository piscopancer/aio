//! Tauri IPC: commands in, typed events out, plus the launch flags.

pub mod cli;
pub mod commands;
pub mod discord;
pub mod events;
#[cfg(windows)]
mod icons;
#[cfg(windows)]
pub mod media_session;
#[cfg(windows)]
pub mod taskbar;
#[cfg(windows)]
pub mod tray;
pub mod window;
