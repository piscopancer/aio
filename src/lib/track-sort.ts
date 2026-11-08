import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { PlaybackContext, Track } from '@/bindings'

/** Columns a song table can be sorted by. */
export const TRACK_SORT_KEYS = ['title', 'album', 'duration'] as const
export type TrackSortKey = (typeof TRACK_SORT_KEYS)[number]

export type TrackSort = { key: TrackSortKey; direction: 'asc' | 'desc' }

/** The kind of page a song table sits on, each of which remembers its own sort. */
type PageKind = PlaybackContext['kind']

/** Sort of each kind of page, kept across restarts; a kind without one shows its page's own order. */
export const useTrackSorts = create<Partial<Record<PageKind, TrackSort>>>()(
  persist(() => ({}), { name: 'aio-track-sort' }),
)

/** Sort a click on the `key` header leads to: descending first, then ascending, then back to the page's own order. */
export function cycleTrackSort(kind: PageKind, key: TrackSortKey) {
  useTrackSorts.setState((sorts) => {
    const current = sorts[kind]
    const next: TrackSort | undefined =
      current?.key !== key
        ? { key, direction: 'desc' }
        : current.direction === 'desc'
          ? { key, direction: 'asc' }
          : undefined
    return { ...sorts, [kind]: next }
  })
}

/** Text order that ignores case and accents and puts "2" before "10". */
const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true })

/** Ascending order of two tracks by each column; an album keeps its songs in disc and track order. */
const ascending = {
  title: (a, b) => collator.compare(a.title, b.title),
  album: (a, b) =>
    collator.compare(a.album, b.album) || (a.discNo ?? 0) - (b.discNo ?? 0) || (a.trackNo ?? 0) - (b.trackNo ?? 0),
  duration: (a, b) => a.durationMs - b.durationMs,
} satisfies Record<TrackSortKey, (a: Track, b: Track) => number>

/** The tracks in `sort`'s order, or as given when there is no sort; ties keep the page's own order. */
export function sortTracks(tracks: Track[], sort: TrackSort | undefined) {
  if (sort === undefined) return tracks
  const sign = sort.direction === 'asc' ? 1 : -1
  return tracks.toSorted((a, b) => sign * ascending[sort.key](a, b))
}
