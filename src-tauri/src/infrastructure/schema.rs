// @generated automatically by Diesel CLI.

diesel::table! {
  album_artists (album_id, artist_id) {
    album_id -> Integer,
    artist_id -> Integer,
    position -> Integer,
  }
}

diesel::table! {
  albums (id) {
    id -> Integer,
    title -> Text,
    year -> Nullable<Integer>,
    cover -> Nullable<Text>,
    pin -> Nullable<Integer>,
    release_type -> Nullable<Text>,
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
  downloads (id) {
    id -> Integer,
    request -> Text,
    folder -> Text,
    file_name -> Nullable<Text>,
    state -> Text,
    track_id -> Nullable<Integer>,
    error -> Nullable<Text>,
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
  player_session (id) {
    id -> Integer,
    queue -> Text,
    position_ms -> Integer,
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
    album_id -> Nullable<Integer>,
    track_no -> Nullable<Integer>,
    disc_no -> Nullable<Integer>,
    duration_ms -> Integer,
    modified -> BigInt,
    cover -> Nullable<Text>,
  }
}

diesel::table! {
  track_genres (track_id, name) {
    track_id -> Integer,
    name -> Text,
    position -> Integer,
  }
}

diesel::table! {
  lyrics (track_id) {
    track_id -> Integer,
    kind -> Text,
    text -> Nullable<Text>,
    lrclib_id -> Nullable<Integer>,
    length_ms -> Integer,
    checked_at -> BigInt,
  }
}

diesel::joinable!(album_artists -> albums (album_id));
diesel::joinable!(album_artists -> artists (artist_id));
diesel::joinable!(lyrics -> tracks (track_id));
diesel::joinable!(playlist_tracks -> playlists (playlist_id));
diesel::joinable!(playlist_tracks -> tracks (track_id));
diesel::joinable!(track_artists -> artists (artist_id));
diesel::joinable!(track_artists -> tracks (track_id));
diesel::joinable!(track_genres -> tracks (track_id));
diesel::joinable!(tracks -> albums (album_id));
diesel::joinable!(tracks -> folders (folder_id));

diesel::allow_tables_to_appear_in_same_query!(
  album_artists,
  albums,
  artists,
  download_settings,
  downloads,
  folders,
  lyrics,
  playback_settings,
  player_session,
  playlist_tracks,
  playlists,
  track_artists,
  track_genres,
  tracks,
);
