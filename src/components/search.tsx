import { useQuery } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { SearchIcon, XIcon } from 'lucide-react'
import { type KeyboardEvent, type ReactNode, useDeferredValue, useEffect, useRef, useState } from 'react'
import type { Album, Artist, Playlist, Track } from '@/bindings'
import { Cover } from '@/components/cover'
import { TrackMenu } from '@/components/track-menu'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverAnchor, PopoverContent } from '@/components/ui/popover'
import { Shortcut } from '@/components/ui/shortcut'
import { credit, plural } from '@/lib/format'
import { player, songsContext } from '@/lib/player'
import { queries } from '@/lib/query'
import { type RecentEntry, clearSearches, forgetSearch, rememberSearch, useRecentSearches } from '@/lib/recent-searches'
import { type SearchResults, searchLibrary } from '@/lib/search'
import { cn } from '@/lib/utils'

/** Id of the search field, which Ctrl+F focuses from anywhere. */
export const SEARCH_INPUT_ID = 'library-search'

/** Keys that focus the search, as `useAppShortcuts` handles them. */
export const SEARCH_KEYS = ['Ctrl', 'F'] as const

/** A section of the popover: the matches of one kind, or the recently picked results when the field is empty. */
type Section = keyof SearchResults | 'recent'

/** One row of the popover: what it shows and what choosing it does. */
type Row = {
  key: string
  section: Section
  entry: RecentEntry
  cover: ReactNode
  title: string
  subtitle: string
  activate: () => void
  /** Songs keep their usual right-click menu. */
  wrap?: (row: ReactNode) => ReactNode
}

const sectionTitles = {
  songs: 'Songs',
  artists: 'Artists',
  albums: 'Albums',
  playlists: 'Playlists',
  recent: 'Recent',
} as const satisfies Record<Section, string>

/** Kind of a row in the recent list, where kinds are mixed. */
const kindLabels = {
  songs: 'Song',
  artists: 'Artist',
  albums: 'Album',
  playlists: 'Playlist',
} as const satisfies Record<RecentEntry['kind'], string>

/** Library search in the toolbar; as you type, the best matches of each kind open in a popover under the field, and an empty field shows the results picked recently. */
export function Search() {
  const [query, setQuery] = useState('')
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const anchorRef = useRef<HTMLLabelElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  // matching runs a render behind the keystrokes, so typing stays smooth on a large library
  const rows = useRows(useDeferredValue(query), () => {
    setOpen(false)
    setQuery('')
    inputRef.current?.blur()
  })
  const searching = query.trim() !== ''
  const shown = open

  useEffect(() => {
    listRef.current?.querySelector('[data-active=true]')?.scrollIntoView({ block: 'nearest' })
  }, [active])

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key]
    if (step !== undefined && rows.length > 0) {
      event.preventDefault()
      setOpen(true)
      setActive((index) => (index + step + rows.length) % rows.length)
    } else if (event.key === 'Enter') {
      rows[active]?.activate()
    } else if (event.key === 'Escape') {
      if (shown) setOpen(false)
      else inputRef.current?.blur()
    }
  }

  return (
    <Popover open={shown} onOpenChange={setOpen}>
      <PopoverAnchor asChild>
        <label ref={anchorRef} className="relative flex w-full max-w-md items-center self-center">
          <SearchIcon className="pointer-events-none absolute left-3 size-5 text-muted-foreground" />
          <Input
            ref={inputRef}
            id={SEARCH_INPUT_ID}
            value={query}
            placeholder="Search..."
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => {
              setQuery(event.target.value)
              setActive(0)
              setOpen(true)
            }}
            onFocus={() => {
              setActive(0)
              setOpen(true)
            }}
            onKeyDown={onKeyDown}
            onBlur={(event) => {
              // the results belong to the focused field, but a song's right-click menu opened from them keeps them
              const next = event.relatedTarget
              if (!(next instanceof Element && next.closest('[role=menu]'))) setOpen(false)
            }}
            className="peer h-10 rounded-full bg-surface pr-24 pl-11 focus:pr-4 focus-visible:bg-surface-hover"
          />
          <Shortcut keys={SEARCH_KEYS} className="pointer-events-none absolute right-3 peer-focus:hidden" />
        </label>
      </PopoverAnchor>
      <PopoverContent
        align="start"
        // typing goes on in the field while the results are open
        onOpenAutoFocus={(event) => event.preventDefault()}
        // a press on the results leaves the focus in the field, so they stay open until the click lands
        onMouseDown={(event) => event.preventDefault()}
        onInteractOutside={(event) => {
          if (event.target instanceof Node && anchorRef.current?.contains(event.target)) event.preventDefault()
        }}
        className="max-h-[min(36rem,calc(100dvh-6rem))] w-(--radix-popover-trigger-width) overflow-y-auto"
      >
        <div ref={listRef} role="listbox" aria-label={searching ? 'Search results' : 'Recent searches'}>
          {rows.length === 0 ? (
            <p className="px-3 py-6 text-center text-sm text-muted-foreground">
              {searching
                ? `Nothing matches “${query.trim()}”`
                : 'Songs, artists, albums and playlists you open from search show up here'}
            </p>
          ) : (
            rows.map((row, index) => {
              const recent = row.section === 'recent'
              const item = (
                <div
                  role="option"
                  aria-selected={index === active}
                  data-active={index === active}
                  onMouseMove={() => setActive(index)}
                  onClick={row.activate}
                  className="group flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 data-[active=true]:bg-foreground/10"
                >
                  <div className="size-10 shrink-0 *:size-full">{row.cover}</div>
                  <div className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate">{row.title}</span>
                    {row.subtitle !== '' && (
                      <span className="truncate text-sm text-muted-foreground">{row.subtitle}</span>
                    )}
                  </div>
                  {recent && (
                    <Button
                      variant="ghost"
                      size="icon-sm"
                      aria-label={`Remove ${row.title} from recent searches`}
                      className="opacity-0 group-hover:opacity-100 group-data-[active=true]:opacity-100"
                      onClick={(event) => {
                        event.stopPropagation()
                        forgetSearch(row.entry)
                      }}
                    >
                      <XIcon />
                    </Button>
                  )}
                </div>
              )
              return (
                <div key={row.key}>
                  {rows[index - 1]?.section !== row.section && (
                    <div className={cn('flex items-center justify-between px-2 pb-1', index > 0 && 'pt-3')}>
                      <h3 className="text-xs font-semibold text-muted-foreground uppercase">
                        {sectionTitles[row.section]}
                      </h3>
                      {recent && (
                        <button
                          type="button"
                          onClick={clearSearches}
                          className="cursor-pointer text-xs font-semibold text-muted-foreground hover:text-foreground"
                        >
                          Clear
                        </button>
                      )}
                    </div>
                  )}
                  {row.wrap === undefined ? item : row.wrap(item)}
                </div>
              )
            })
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}

/** Rows of the popover: matches of the query in section order, or the recent picks that still exist when the query is empty; choosing a row remembers it, then runs `done`. */
function useRows(query: string, done: () => void) {
  const navigate = useNavigate()
  const recent = useRecentSearches((state) => state.entries)
  const { data: tracks = [] } = useQuery(queries.tracks())
  const { data: albums = [] } = useQuery(queries.albums())
  const { data: artists = [] } = useQuery(queries.artists())
  const { data: playlists = [] } = useQuery(queries.playlists())

  const choose = (entry: RecentEntry, action: () => unknown) => () => {
    rememberSearch(entry)
    void action()
    done()
  }
  const builders = {
    songs: (track: Track, section: Section): Row => {
      const entry = { kind: 'songs', id: track.id } as const
      return {
        key: `${section}:song:${track.id}`,
        section,
        entry,
        cover: <Cover src={track.cover} kind="track" className="rounded-sm" />,
        title: track.title,
        subtitle: credit(track.artists),
        activate: choose(entry, () => player.play(songsContext, track.id)),
        wrap: (row) => (
          <TrackMenu track={track} context={songsContext}>
            {row}
          </TrackMenu>
        ),
      }
    },
    artists: (artist: Artist, section: Section): Row => {
      const entry = { kind: 'artists', id: artist.id } as const
      return {
        key: `${section}:artist:${artist.id}`,
        section,
        entry,
        cover: <Cover src={artist.cover} kind="artist" />,
        title: artist.name,
        subtitle: plural(artist.trackCount, 'song'),
        activate: choose(entry, () => navigate({ to: '/artists/$artistId', params: { artistId: artist.id } })),
      }
    },
    albums: (album: Album, section: Section): Row => {
      const entry = { kind: 'albums', id: album.id } as const
      return {
        key: `${section}:album:${album.id}`,
        section,
        entry,
        cover: <Cover src={album.cover} kind="album" className="rounded-sm" />,
        title: album.title,
        subtitle: [album.year, album.artist].filter((part) => part !== null).join(' • '),
        activate: choose(entry, () => navigate({ to: '/albums/$albumId', params: { albumId: album.id } })),
      }
    },
    playlists: (playlist: Playlist, section: Section): Row => {
      const entry = { kind: 'playlists', id: playlist.id } as const
      return {
        key: `${section}:playlist:${playlist.id}`,
        section,
        entry,
        cover: <Cover src={playlist.cover} kind="playlist" className="rounded-sm" />,
        title: playlist.name,
        subtitle: plural(playlist.trackCount, 'song'),
        activate: choose(entry, () => navigate({ to: '/playlists/$playlistId', params: { playlistId: playlist.id } })),
      }
    },
  }

  const results = searchLibrary(query, { tracks, albums, artists, playlists })
  if (results !== null) {
    return [
      ...results.songs.map((track) => builders.songs(track, 'songs')),
      ...results.artists.map((artist) => builders.artists(artist, 'artists')),
      ...results.albums.map((album) => builders.albums(album, 'albums')),
      ...results.playlists.map((playlist) => builders.playlists(playlist, 'playlists')),
    ]
  }

  // a picked item may have left the library since; it is skipped until it is picked again
  return recent.flatMap((entry): Row[] => {
    const row = (() => {
      switch (entry.kind) {
        case 'songs': {
          const track = tracks.find((item) => item.id === entry.id)
          return track && builders.songs(track, 'recent')
        }
        case 'artists': {
          const artist = artists.find((item) => item.id === entry.id)
          return artist && builders.artists(artist, 'recent')
        }
        case 'albums': {
          const album = albums.find((item) => item.id === entry.id)
          return album && builders.albums(album, 'recent')
        }
        case 'playlists': {
          const playlist = playlists.find((item) => item.id === entry.id)
          return playlist && builders.playlists(playlist, 'recent')
        }
        default:
          return entry satisfies never
      }
    })()
    if (row === undefined) return []
    const subtitle = [kindLabels[entry.kind], row.subtitle].filter((part) => part !== '').join(' • ')
    return [{ ...row, subtitle }]
  })
}
