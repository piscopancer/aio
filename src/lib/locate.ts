import { linkOptions, useNavigate } from '@tanstack/react-router'
import { create } from 'zustand'
import type { PlaybackContext, TrackId } from '@/bindings'
import { contextKey, currentItem, songsContext, usePlayer } from '@/lib/player'

/** The page of the library a playback context is started from. */
export function contextLink(context: PlaybackContext) {
  switch (context.kind) {
    case 'songs':
      return linkOptions({ to: '/songs' })
    case 'album':
      return linkOptions({ to: '/albums/$albumId', params: { albumId: context.id } })
    case 'artist':
      return linkOptions({ to: '/artists/$artistId', params: { artistId: context.id } })
    case 'playlist':
      return linkOptions({ to: '/playlists/$playlistId', params: { playlistId: context.id } })
    case 'folder':
      return linkOptions({ to: '/folders/$folderId', params: { folderId: context.id } })
    case 'recent':
    case 'added':
      return linkOptions({ to: '/' })
    default:
      return context satisfies never
  }
}

/** A song waiting to be shown by the song table of the context it plays from, keyed by `contextKey`; the table
 * selects it, centres it and clears the request. */
export const useLocateRequest = create<{ request: { key: string; trackId: TrackId } | null }>(() => ({
  request: null,
}))

/** Opens the page the playing song was started from, where its table selects the song and centres it in the list. */
export function useLocatePlaying() {
  const navigate = useNavigate()
  return () => {
    const state = usePlayer.getState()
    const track = currentItem(state)?.track
    if (track === undefined) return
    // songs added to the queue while nothing played have no view of their own; the library is theirs
    const context = state.context ?? songsContext
    useLocateRequest.setState({ request: { key: contextKey(context), trackId: track.id } })
    void navigate(contextLink(context))
  }
}
