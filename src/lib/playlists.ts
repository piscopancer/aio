import { useMutation, useQuery } from '@tanstack/react-query'
import { CalendarPlusIcon, HistoryIcon, type LucideIcon, Music2Icon } from 'lucide-react'
import type { Playlist } from '@/bindings'
import { type PlaylistOrder, useLayout } from '@/lib/layout'
import { splitPinned } from '@/lib/pins'
import { mutations, queries } from '@/lib/query'

/** What each order is called in its menu and hint, the icon beside it, and how it ranks two playlists; ties keep the newest first, as they arrive. */
export const ORDERS = {
  played: {
    label: 'Recently played',
    icon: HistoryIcon,
    compare: (a: Playlist, b: Playlist) => (b.playedAt ?? -1) - (a.playedAt ?? -1),
  },
  created: {
    label: 'Date created',
    icon: CalendarPlusIcon,
    compare: (a: Playlist, b: Playlist) => b.createdAt - a.createdAt,
  },
  size: { label: 'Most songs', icon: Music2Icon, compare: (a: Playlist, b: Playlist) => b.trackCount - a.trackCount },
} satisfies Record<PlaylistOrder, { label: string; icon: LucideIcon; compare: (a: Playlist, b: Playlist) => number }>

/** The playlists as the sidebar lists them: the pinned ones in the order set by hand, then the rest in `order`. */
export function inListOrder(playlists: readonly Playlist[], order: PlaylistOrder) {
  const { pinned, rest } = splitPinned(playlists)
  return { pinned, rest: rest.toSorted(ORDERS[order].compare) }
}

/** Every playlist in the order the sidebar shows them, for the lists that offer playlists to pick. */
export function useOrderedPlaylists() {
  const { data: playlists = [] } = useQuery(queries.playlists())
  const order = useLayout((state) => state.playlistOrder)
  const { pinned, rest } = inListOrder(playlists, order)
  return [...pinned, ...rest]
}

/** Numbered default name of the next playlist, which the user can change right away or later. */
export function useNextPlaylistName() {
  const { data: playlists = [] } = useQuery(queries.playlists())
  return `My Playlist #${playlists.length + 1}`
}

/** Creates a playlist with the default name and no image at once, for flows that cannot stop for a dialog. */
export function useCreatePlaylist() {
  const name = useNextPlaylistName()
  const { mutateAsync } = useMutation(mutations.createPlaylist())
  return async () => {
    const id = await mutateAsync({ name, ownCover: null })
    return { id, name }
  }
}
