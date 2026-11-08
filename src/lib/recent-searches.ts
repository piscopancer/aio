import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { AlbumId, ArtistId, PlaylistId, TrackId } from '@/bindings'

/** A search result the user picked, remembered by kind and id so it shows its current name and cover. */
export type RecentEntry =
  | { kind: 'songs'; id: TrackId }
  | { kind: 'artists'; id: ArtistId }
  | { kind: 'albums'; id: AlbumId }
  | { kind: 'playlists'; id: PlaylistId }

/** How many picked results the search remembers. */
const RECENT_LIMIT = 8

const same = (a: RecentEntry, b: RecentEntry) => a.kind === b.kind && a.id === b.id

const initialRecent: { entries: RecentEntry[] } = { entries: [] }

/** Results picked from the search, newest first, kept across restarts. */
export const useRecentSearches = create<typeof initialRecent>()(
  persist(() => initialRecent, { name: 'aio-recent-searches' }),
)

/** Moves the entry to the top, adding it when new and dropping the oldest past the limit. */
export function rememberSearch(entry: RecentEntry) {
  useRecentSearches.setState(({ entries }) => ({
    entries: [entry, ...entries.filter((other) => !same(other, entry))].slice(0, RECENT_LIMIT),
  }))
}

export function forgetSearch(entry: RecentEntry) {
  useRecentSearches.setState(({ entries }) => ({ entries: entries.filter((other) => !same(other, entry)) }))
}

export function clearSearches() {
  useRecentSearches.setState({ entries: [] })
}
