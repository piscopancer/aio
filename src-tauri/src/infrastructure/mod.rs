//! Adapters to SQLite, the file system, the sound device and the download tools that implement the application ports.

pub mod audio;
pub mod covers;
mod fader;
pub mod files;
pub mod lrclib;
pub mod media;
pub mod oklch;
pub mod opus;
mod schema;
pub mod sqlite;
pub mod tag_sources;
pub mod tools;
