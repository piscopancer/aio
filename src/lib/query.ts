import { MutationCache, QueryCache, QueryClient, mutationOptions, queryOptions } from '@tanstack/react-query'
import { getVersion } from '@tauri-apps/api/app'
import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart'
import { readText } from '@tauri-apps/plugin-clipboard-manager'
import {
  type AlbumId,
  type ArtistId,
  type DownloadId,
  type DownloadRequest,
  type FolderId,
  type LyricsOrigin,
  type PinTarget,
  type Playlist,
  type PlaylistDetail,
  type PlaylistId,
  type TagSearchQuery,
  type TrackEdit,
  type TrackId,
  type WindowSettings,
} from '@/bindings'
import { toast } from 'sonner'
import { reportError } from '@/lib/errors'
import { plural } from '@/lib/format'
import { type CommandError, commands, unwrap } from '@/lib/ipc'

declare module '@tanstack/react-query' {
  interface Register {
    defaultError: CommandError
    /** `inline` queries show their error where they are used instead of as a toast. */
    queryMeta: { inline?: boolean }
  }
}

/** Library data is made stale by backend events, so nothing refetches on its own. */
export const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: Infinity, retry: false } },
  queryCache: new QueryCache({
    onError: (error, query) => {
      if (query.meta?.inline !== true) reportError(error)
    },
  }),
  mutationCache: new MutationCache({ onError: (error) => reportError(error) }),
})

/** How long the results of a search by name for a download count as fresh. */
const SEARCH_FRESH_MS = 5 * 60 * 1000

/** How long a streamed audio address is reused; the site lets it play for about six hours. */
const STREAM_FRESH_MS = 60 * 60 * 1000

/** Key roots; the library root covers every view a scan can change, playlists included. */
const roots = {
  library: ['library'],
  playlists: ['library', 'playlists'],
  downloads: ['downloads'],
} as const

export const queries = {
  appVersion: () => queryOptions({ queryKey: ['appVersion'], queryFn: () => getVersion() }),
  /** Whether Windows starts the app at sign-in, as its startup list holds it; the user may change it there too. */
  autostart: () => queryOptions({ queryKey: ['autostart'], queryFn: () => isEnabled(), staleTime: 0 }),
  windowSettings: () => queryOptions({ queryKey: ['windowSettings'], queryFn: () => commands.getWindowSettings() }),
  scenario: () => queryOptions({ queryKey: ['scenario'], queryFn: () => commands.getScenario() }),
  folders: () =>
    queryOptions({ queryKey: [...roots.library, 'folders'], queryFn: () => unwrap(commands.listFolders()) }),
  folder: (id: FolderId) =>
    queryOptions({ queryKey: [...roots.library, 'folders', id], queryFn: () => unwrap(commands.getFolder(id)) }),
  tracks: () => queryOptions({ queryKey: [...roots.library, 'tracks'], queryFn: () => unwrap(commands.listTracks()) }),
  albums: () => queryOptions({ queryKey: [...roots.library, 'albums'], queryFn: () => unwrap(commands.listAlbums()) }),
  album: (id: AlbumId) =>
    queryOptions({ queryKey: [...roots.library, 'albums', id], queryFn: () => unwrap(commands.getAlbum(id)) }),
  artists: () =>
    queryOptions({ queryKey: [...roots.library, 'artists'], queryFn: () => unwrap(commands.listArtists()) }),
  genres: () => queryOptions({ queryKey: [...roots.library, 'genres'], queryFn: () => unwrap(commands.listGenres()) }),
  artist: (id: ArtistId) =>
    queryOptions({ queryKey: [...roots.library, 'artists', id], queryFn: () => unwrap(commands.getArtist(id)) }),
  playlists: () => queryOptions({ queryKey: [...roots.playlists], queryFn: () => unwrap(commands.listPlaylists()) }),
  /** What was played last and the songs added last; under the library root, as it lists the library's songs. */
  home: () => queryOptions({ queryKey: [...roots.library, 'home'], queryFn: () => unwrap(commands.getHome()) }),
  playlist: (id: PlaylistId) =>
    queryOptions({ queryKey: [...roots.playlists, id], queryFn: () => unwrap(commands.getPlaylist(id)) }),
  /** The playlists that already hold the song; under the playlists root, so adding or removing songs refetches it. */
  playlistsWithTrack: (trackId: TrackId) =>
    queryOptions({
      queryKey: [...roots.playlists, 'withTrack', trackId],
      queryFn: () => unwrap(commands.playlistsWithTrack(trackId)),
    }),
  /** Every download of the session; `DownloadsChanged` events replace it. */
  downloads: () => queryOptions({ queryKey: [...roots.downloads, 'list'], queryFn: () => commands.listDownloads() }),
  /** Versions of the download tools; `ToolsChanged` events replace it. */
  downloadTools: () =>
    queryOptions({ queryKey: [...roots.downloads, 'tools'], queryFn: () => unwrap(commands.getDownloadTools()) }),
  /** Installs missing tools once per session, the first time a download dialog opens. */
  prepareDownloads: () =>
    queryOptions({
      queryKey: [...roots.downloads, 'prepare'],
      queryFn: () => unwrap(commands.prepareDownloads()),
      meta: { inline: true },
    }),
  downloadSettings: () =>
    queryOptions({ queryKey: [...roots.downloads, 'settings'], queryFn: () => commands.getDownloadSettings() }),
  /** Where a download folder stands relative to the library; null is the folder from the settings. */
  downloadFolder: (folder: string | null) =>
    queryOptions({
      queryKey: [...roots.downloads, 'folder', folder],
      queryFn: () => unwrap(commands.checkDownloadFolder(folder)),
      staleTime: 0,
    }),
  /** Text on the clipboard, or null when it holds none; read again while the download dialog waits for a link. */
  clipboardText: () =>
    queryOptions({
      queryKey: [...roots.downloads, 'clipboard'],
      // reading fails when the clipboard holds an image or nothing at all
      queryFn: () => readText().catch(() => null),
      staleTime: 0,
    }),
  /** A few harmonious dark colours of a cover image, strongest first, which the lyrics background and the page headers flow with; a missing image gives none. */
  coverPalette: (file: string | null) =>
    queryOptions({
      queryKey: ['coverPalette', file],
      queryFn: () => (file === null ? null : unwrap(commands.coverPalette(file))),
      meta: { inline: true },
    }),
  /** Lyrics of a song; finding them once is enough, as they only ever move from LRCLIB into the file. */
  lyrics: (id: TrackId) =>
    queryOptions({
      queryKey: ['lyrics', id],
      queryFn: () => unwrap(commands.trackLyrics(id)),
      staleTime: Infinity,
      retry: false,
      meta: { inline: true },
    }),
  /** What LRCLIB has for a song whose file holds other lyrics, null when it has nothing different. */
  lrclibLyrics: (id: TrackId) =>
    queryOptions({
      // under the key of the song's lyrics, so whatever refreshes those refreshes this as well
      queryKey: ['lyrics', id, 'lrclib'],
      queryFn: () => unwrap(commands.lrclibLyrics(id)),
      staleTime: Infinity,
      retry: false,
      meta: { inline: true },
    }),
  /** Tags of a song file for the edit dialog, read afresh each time it opens. */
  trackTags: (id: TrackId) =>
    queryOptions({
      queryKey: [...roots.library, 'tags', id],
      queryFn: () => unwrap(commands.trackTags(id)),
      staleTime: 0,
      meta: { inline: true },
    }),
  downloadPreview: (link: string) =>
    queryOptions({
      queryKey: [...roots.downloads, 'preview', link],
      queryFn: () => unwrap(commands.previewDownload(link)),
      meta: { inline: true },
    }),
  /** Songs and videos on YouTube Music found by a typed name for the download dialog; kept a while, so going back to the results shows them at once. */
  downloadSearch: (query: string) =>
    queryOptions({
      queryKey: [...roots.downloads, 'search', query],
      queryFn: () => unwrap(commands.searchDownloads(query)),
      staleTime: SEARCH_FRESH_MS,
      meta: { inline: true },
    }),
  /** Direct address of a found song's audio to listen to before downloading; the address expires after a few hours, so it is kept for one. */
  downloadStream: (link: string) =>
    queryOptions({
      queryKey: [...roots.downloads, 'stream', link],
      queryFn: () => unwrap(commands.streamDownload(link)),
      staleTime: STREAM_FRESH_MS,
    }),
}

/** Marks every library view stale, used when the backend reports a library change. */
export function invalidateLibrary() {
  return queryClient.invalidateQueries({ queryKey: roots.library })
}

/** Refetches the playlists, as after one of them changed or was played. */
export function invalidatePlaylists() {
  return queryClient.invalidateQueries({ queryKey: roots.playlists })
}

/** Puts the window settings the backend saved into the cache. */
const keepWindowSettings = (settings: WindowSettings) =>
  queryClient.setQueryData(queries.windowSettings().queryKey, settings)

export const mutations = {
  setAutostart: () =>
    mutationOptions({
      mutationFn: (on: boolean) => (on ? enable() : disable()),
      onSuccess: (_, on) => queryClient.setQueryData(queries.autostart().queryKey, on),
    }),
  setCloseToTray: () =>
    mutationOptions({
      mutationFn: (on: boolean) => unwrap(commands.setCloseToTray(on)),
      onSuccess: keepWindowSettings,
    }),
  setStartHidden: () =>
    mutationOptions({
      mutationFn: (on: boolean) => unwrap(commands.setStartHidden(on)),
      onSuccess: keepWindowSettings,
    }),
  addFolder: () => mutationOptions({ mutationFn: (path: string) => unwrap(commands.addFolder(path)) }),
  removeFolder: () => mutationOptions({ mutationFn: (path: string) => unwrap(commands.removeFolder(path)) }),
  setFolderHidden: () =>
    mutationOptions({
      mutationFn: ({ path, hidden }: { path: string; hidden: boolean }) =>
        unwrap(commands.setFolderHidden(path, hidden)),
    }),
  scan: () => mutationOptions({ mutationFn: () => unwrap(commands.scanLibrary()) }),
  setDownloadFolder: () =>
    mutationOptions({
      mutationFn: (folder: string | null) => unwrap(commands.setDownloadFolder(folder)),
      onSuccess: (settings) => {
        queryClient.setQueryData(queries.downloadSettings().queryKey, settings)
        return queryClient.invalidateQueries({ queryKey: [...roots.downloads, 'folder'] })
      },
    }),
  updateDownloadTools: () => mutationOptions({ mutationFn: () => unwrap(commands.updateDownloadTools()) }),
  startDownload: () =>
    mutationOptions({
      mutationFn: (request: DownloadRequest) => unwrap(commands.startDownload(request)),
      onSuccess: (_, { title }) => toast(`Downloading ${title}`),
    }),
  dismissDownload: () => mutationOptions({ mutationFn: (id: DownloadId) => commands.dismissDownload(id) }),
  cancelDownload: () => mutationOptions({ mutationFn: (id: DownloadId) => commands.cancelDownload(id) }),
  createPlaylist: () =>
    mutationOptions({
      mutationFn: ({ name, ownCover }: Pick<Playlist, 'name' | 'ownCover'>) =>
        unwrap(commands.createPlaylist(name, ownCover)),
      onSettled: invalidatePlaylists,
    }),
  editPlaylist: () =>
    mutationOptions({
      mutationFn: ({ id, name, ownCover }: Pick<Playlist, 'id' | 'name' | 'ownCover'>) =>
        unwrap(commands.editPlaylist(id, name, ownCover)),
      onSettled: invalidatePlaylists,
    }),
  /** Stores a square copy of a picked image file and resolves to the copy's path, ready to become a cover. */
  importCover: () => mutationOptions({ mutationFn: (file: string) => unwrap(commands.importCover(file)) }),
  /** Variants of a song's tags from online music databases, looked up by the tags its edit form holds now. */
  searchTags: () =>
    mutationOptions({
      mutationFn: ({ id, query }: { id: TrackId; query: TagSearchQuery }) => unwrap(commands.searchTags(id, query)),
    }),
  /** Downloads a cover the tag search found and stores it, giving the path the edit form sets. */
  importFoundCover: () => mutationOptions({ mutationFn: (url: string) => unwrap(commands.importFoundCover(url)) }),
  editTrack: () =>
    mutationOptions({
      mutationFn: ({ id, edit }: { id: TrackId; edit: TrackEdit }) => unwrap(commands.editTrack(id, edit)),
      onSuccess: (_, { edit }) => toast.success(`Saved ${edit.title.trim() || 'the song'}`),
      // the lyrics live outside the library queries, and an edit may have changed them
      onSettled: (_, __, { id }) =>
        Promise.all([invalidateLibrary(), queryClient.invalidateQueries({ queryKey: queries.lyrics(id).queryKey })]),
    }),
  /** Writes lyrics into the song's file over those it holds. */
  saveLyrics: () =>
    mutationOptions({
      mutationFn: ({ id, text, origin }: { id: TrackId; text: string; origin: LyricsOrigin }) =>
        unwrap(commands.saveLyrics(id, text, origin)),
      onSettled: (_, __, { id }) => queryClient.invalidateQueries({ queryKey: queries.lyrics(id).queryKey }),
    }),
  deletePlaylist: () =>
    mutationOptions({
      mutationFn: (id: PlaylistId) => unwrap(commands.deletePlaylist(id)),
      onSettled: invalidatePlaylists,
    }),
  pin: () =>
    mutationOptions({ mutationFn: (target: PinTarget) => unwrap(commands.pin(target)), onSettled: invalidateLibrary }),
  unpin: () =>
    mutationOptions({
      mutationFn: (target: PinTarget) => unwrap(commands.unpin(target)),
      onSettled: invalidateLibrary,
    }),
  /** Moves a pin optimistically, so the dropped item does not jump back while the backend saves. */
  movePin: () =>
    mutationOptions({
      mutationFn: ({ target, to }: { target: PinTarget; to: number }) => unwrap(commands.movePin(target, to)),
      onMutate: async ({ target, to }) => {
        await queryClient.cancelQueries({ queryKey: roots.library })
        switch (target.kind) {
          case 'playlist':
            return void queryClient.setQueryData(
              queries.playlists().queryKey,
              (list) => list && repin(list, target.id, to),
            )
          case 'album':
            return void queryClient.setQueryData(
              queries.albums().queryKey,
              (list) => list && repin(list, target.id, to),
            )
          case 'artist':
            return void queryClient.setQueryData(
              queries.artists().queryKey,
              (list) => list && repin(list, target.id, to),
            )
          default:
            return target satisfies never
        }
      },
      onSettled: invalidateLibrary,
    }),
  /** Adds tracks the playlist lacks and tells how many it already had. */
  addToPlaylist: () =>
    mutationOptions({
      mutationFn: ({ id, trackIds }: { id: PlaylistId; name: string; trackIds: TrackId[] }) =>
        unwrap(commands.addToPlaylist(id, trackIds)),
      onSuccess: ({ added, skipped }, { name }) => {
        if (skipped === 0) toast.success(`Added to ${name}`)
        else if (added === 0) toast.info(`Already in ${name}`)
        else toast.success(`Added ${plural(added, 'song')} to ${name}, skipped ${skipped} already there`)
      },
      onSettled: invalidatePlaylists,
    }),
  removeFromPlaylist: () =>
    mutationOptions({
      mutationFn: ({ id, trackIds }: { id: PlaylistId; trackIds: TrackId[] }) =>
        unwrap(commands.removeFromPlaylist(id, trackIds)),
      onSettled: invalidatePlaylists,
    }),
  /** Reorders optimistically, so the dropped row does not jump back while the backend saves. */
  movePlaylistTrack: () =>
    mutationOptions({
      mutationFn: ({ id, trackId, to }: { id: PlaylistId; trackId: TrackId; to: number }) =>
        unwrap(commands.movePlaylistTrack(id, trackId, to)),
      onMutate: async ({ id, trackId, to }) => {
        const { queryKey } = queries.playlist(id)
        await queryClient.cancelQueries({ queryKey })
        queryClient.setQueryData(queryKey, (detail) => detail && reorder(detail, trackId, to))
      },
      onSettled: invalidatePlaylists,
    }),
}

/** Same move the pins make: take the item out of the pinned order, then insert it at `to` of the rest. */
function repin<const T extends { id: number; pin: number | null }>(list: T[], id: number, to: number) {
  const order = list
    .filter((item) => item.pin !== null && item.id !== id)
    .toSorted((a, b) => (a.pin ?? 0) - (b.pin ?? 0))
    .map((item) => item.id)
  order.splice(to, 0, id)
  return list.map((item) => (item.pin === null && item.id !== id ? item : { ...item, pin: order.indexOf(item.id) }))
}

/** Same move the playlist aggregate makes: take the track out, then insert it at `to` of the remaining order. */
function reorder(detail: PlaylistDetail, trackId: TrackId, to: number) {
  const moved = detail.tracks.find((track) => track.id === trackId)
  if (moved === undefined) return detail
  const tracks = detail.tracks.filter((track) => track.id !== trackId)
  tracks.splice(Math.min(to, tracks.length), 0, moved)
  return { ...detail, tracks }
}
