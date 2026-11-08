use serde::{Deserialize, Serialize};
use specta::Type;

/// Declares identities of library records; they cross IPC as plain numbers.
macro_rules! id {
  ($($(#[$doc:meta])* $name:ident),* $(,)?) => {$(
    $(#[$doc])*
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Type)]
    pub struct $name(pub u32);
  )*};
}

id!(
  /// An artist, either of tracks or of albums.
  ArtistId,
  /// An album, unique per title and album artist.
  AlbumId,
  /// One audio file of the library.
  TrackId,
  /// A user playlist.
  PlaylistId,
  /// A folder of the library.
  FolderId,
  /// One download started in this session.
  DownloadId,
);
