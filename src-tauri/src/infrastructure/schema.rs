// @generated automatically by Diesel CLI.

diesel::table! {
  albums (id) {
    id -> Integer,
    title -> Text,
    artist_id -> Integer,
    year -> Nullable<Integer>,
    cover -> Nullable<Text>,
  }
}

diesel::table! {
  artists (id) {
    id -> Integer,
    name -> Text,
  }
}

diesel::table! {
  download_settings (id) {
    id -> Integer,
    folder -> Nullable<Text>,
    tools_checked_at -> Nullable<BigInt>,
  }
}

diesel::table! {
  folders (id) {
    id -> Integer,
    path -> Text,
  }
}

diesel::table! {
  playback_settings (id) {
    id -> Integer,
    volume -> Integer,
    shuffle -> Bool,
    repeat -> Text,
  }
}

diesel::table! {
  playlist_tracks (playlist_id, track_id) {
    playlist_id -> Integer,
    track_id -> Integer,
    position -> Integer,
  }
}

diesel::table! {
  playlists (id) {
    id -> Integer,
    name -> Text,
    created_at -> BigInt,
    cover -> Nullable<Text>,
  }
}

diesel::table! {
  tracks (id) {
    id -> Integer,
    folder_id -> Integer,
    path -> Text,
    title -> Text,
    artist_id -> Integer,
    album_id -> Integer,
    track_no -> Nullable<Integer>,
    disc_no -> Nullable<Integer>,
    duration_ms -> Integer,
    modified -> BigInt,
  }
}

diesel::joinable!(albums -> artists (artist_id));
diesel::joinable!(playlist_tracks -> playlists (playlist_id));
diesel::joinable!(playlist_tracks -> tracks (track_id));
diesel::joinable!(tracks -> albums (album_id));
diesel::joinable!(tracks -> artists (artist_id));
diesel::joinable!(tracks -> folders (folder_id));

diesel::allow_tables_to_appear_in_same_query!(
  albums,
  artists,
  download_settings,
  folders,
  playback_settings,
  playlist_tracks,
  playlists,
  tracks,
);
