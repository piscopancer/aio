// @generated automatically by Diesel CLI.

diesel::table! {
  albums (id) {
    id -> Integer,
    title -> Text,
    artist_id -> Integer,
    year -> Nullable<Integer>,
    cover -> Nullable<Text>,
    pin -> Nullable<Integer>,
  }
}

diesel::table! {
  artists (id) {
    id -> Integer,
    name -> Text,
    pin -> Nullable<Integer>,
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
    played_at -> Nullable<BigInt>,
    pin -> Nullable<Integer>,
  }
}

diesel::table! {
  track_artists (track_id, artist_id) {
    track_id -> Integer,
    artist_id -> Integer,
    position -> Integer,
  }
}

diesel::table! {
  tracks (id) {
    id -> Integer,
    folder_id -> Nullable<Integer>,
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

diesel::table! {
  lyrics (track_id) {
    track_id -> Integer,
    kind -> Text,
    text -> Nullable<Text>,
    lrclib_id -> Nullable<Integer>,
    length_ms -> Integer,
    in_file -> Bool,
    checked_at -> BigInt,
  }
}

diesel::joinable!(albums -> artists (artist_id));
diesel::joinable!(lyrics -> tracks (track_id));
diesel::joinable!(playlist_tracks -> playlists (playlist_id));
diesel::joinable!(playlist_tracks -> tracks (track_id));
diesel::joinable!(track_artists -> artists (artist_id));
diesel::joinable!(track_artists -> tracks (track_id));
diesel::joinable!(tracks -> albums (album_id));
diesel::joinable!(tracks -> artists (artist_id));
diesel::joinable!(tracks -> folders (folder_id));

diesel::allow_tables_to_appear_in_same_query!(
  albums,
  artists,
  download_settings,
  folders,
  lyrics,
  playback_settings,
  playlist_tracks,
  playlists,
  track_artists,
  tracks,
);
