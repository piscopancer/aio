import { useMutation, useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import {
  ArrowDownWideNarrowIcon,
  CheckIcon,
  LibraryIcon,
  Music2Icon,
  PlusIcon,
  SearchIcon,
  Volume2Icon,
} from 'lucide-react'
import { useState } from 'react'
import type { PlaybackContext, Playlist } from '@/bindings'
import { Cover } from '@/components/cover'
import { Hint } from '@/components/hint'
import { Marquee } from '@/components/marquee'
import { CreatePlaylistDialog } from '@/components/playlist-dialogs'
import { PinBadge } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { PlaylistMenu } from '@/components/playlist-menu'
import { Button } from '@/components/ui/button'
import { ClearButton } from '@/components/ui/clear-button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { carriesTrack, droppedTrack } from '@/lib/dnd'
import {
  PLAYLIST_ORDERS,
  type PlaylistOrder,
  SIDEBAR_KEYS,
  setPlaylistOrder,
  toggleSidebar,
  useLayout,
} from '@/lib/layout'
import { useIsPlayingFrom } from '@/lib/player'
import { mutations, queries } from '@/lib/query'
import { PIN_IDLE, pinDropMarks, splitPinned, usePinReorder } from '@/lib/pins'
import { matcher } from '@/lib/search'
import { cn } from '@/lib/utils'

/** The left column, given over to the playlists. */
export function Sidebar() {
  return (
    <aside className={cn('flex shrink-0 flex-col', useLayout((state) => state.sidebarCompact) ? 'w-20' : 'w-72')}>
      <Library />
    </aside>
  )
}

function Library() {
  const { data: playlists = [] } = useQuery(queries.playlists())
  const [creating, setCreating] = useState(false)
  const compact = useLayout((state) => state.sidebarCompact)
  const order = useLayout((state) => state.playlistOrder)

  return (
    <section className="flex min-h-0 flex-1 flex-col rounded-lg bg-surface">
      <header
        className={cn('flex items-center gap-2 pt-4 pb-2 text-muted-foreground', compact ? 'flex-col px-2' : 'px-4')}
      >
        <Hint label={`${compact ? 'Expand' : 'Collapse'} playlists`} keys={SIDEBAR_KEYS} side="right">
          <Button
            variant="ghost"
            className={cn('h-10 gap-3 px-2 hover:bg-surface-hover [&_svg]:size-6', compact && 'w-10 px-0')}
            onClick={toggleSidebar}
          >
            <LibraryIcon />
            {!compact && <span className="text-base font-semibold">Playlists</span>}
          </Button>
        </Hint>
        <Hint label="Create playlist" side="right">
          <Button
            variant="ghost"
            size="icon"
            className={cn('hover:bg-surface-hover', !compact && 'ml-auto')}
            onClick={() => setCreating(true)}
          >
            <PlusIcon className="size-5" />
          </Button>
        </Hint>
        <CreatePlaylistDialog open={creating} onOpenChange={setCreating} />
      </header>
      {compact ? (
        <div className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-2 pb-2">
          <PlaylistRows playlists={playlists} order={order} compact reorderable />
        </div>
      ) : (
        // mounted anew when the column widens again, so the search starts empty
        <PlaylistList playlists={playlists} order={order} />
      )}
    </section>
  )
}

/** What each order is called in its menu and hint, and how it ranks two playlists; ties keep the newest first, as they arrive. */
const ORDERS = {
  played: {
    label: 'Recently played',
    compare: (a: Playlist, b: Playlist) => (b.playedAt ?? -1) - (a.playedAt ?? -1),
  },
  created: { label: 'Date created', compare: (a: Playlist, b: Playlist) => b.createdAt - a.createdAt },
  size: { label: 'Most songs', compare: (a: Playlist, b: Playlist) => b.trackCount - a.trackCount },
} satisfies Record<PlaylistOrder, { label: string; compare: (a: Playlist, b: Playlist) => number }>

/** The pinned playlists in the order set by hand, which drags change while `reorderable`, then the rest in `order`. */
function PlaylistRows({
  playlists,
  order,
  compact,
  reorderable,
}: {
  playlists: readonly Playlist[]
  order: PlaylistOrder
  compact: boolean
  reorderable: boolean
}) {
  const { pinned, rest } = splitPinned(playlists)
  const dragProps = usePinReorder(
    'playlist',
    pinned.map((playlist) => playlist.id),
    'list',
  )
  return (
    <>
      {pinned.map((playlist) => (
        <PlaylistLink
          key={playlist.id}
          playlist={playlist}
          compact={compact}
          pinDrag={reorderable ? dragProps(playlist.id) : undefined}
        />
      ))}
      {rest.toSorted(ORDERS[order].compare).map((playlist) => (
        <PlaylistLink key={playlist.id} playlist={playlist} compact={compact} />
      ))}
    </>
  )
}

/** The full column: a search over the playlist names and the order picker above the list. */
function PlaylistList({ playlists, order }: { playlists: Playlist[]; order: PlaylistOrder }) {
  const [query, setQuery] = useState('')
  const matches = matcher(query)
  const shown = playlists.filter((playlist) => matches(playlist.name))

  return (
    <>
      {playlists.length > 0 && (
        <div className="flex items-center gap-1 px-4 pb-2">
          <label className="relative flex min-w-0 flex-1 items-center">
            <SearchIcon className="pointer-events-none absolute left-2.5 size-4 text-muted-foreground" />
            <Input
              value={query}
              placeholder="Search..."
              aria-label="Search playlists"
              autoComplete="off"
              spellCheck={false}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => event.key === 'Escape' && setQuery('')}
              className="h-8 bg-surface-hover pr-8 pl-8 focus-visible:bg-secondary"
            />
            {query !== '' && <ClearButton onClear={() => setQuery('')} className="right-1 size-6" />}
          </label>
          <OrderMenu order={order} />
        </div>
      )}
      <div className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-2 pb-2">
        {playlists.length === 0 ? (
          <p className="px-4 py-3 text-sm text-muted-foreground">
            Create a playlist with the + button, then drag songs onto it.
          </p>
        ) : shown.length === 0 ? (
          <p className="px-4 py-3 text-sm text-muted-foreground">No playlists match “{query.trim()}”.</p>
        ) : (
          // the pinned order is set by dragging over the whole list, not a part a search left
          <PlaylistRows playlists={shown} order={order} compact={false} reorderable={query.trim() === ''} />
        )}
      </div>
    </>
  )
}

/** Button that opens the list of orders, the current one checked. */
function OrderMenu({ order }: { order: PlaylistOrder }) {
  return (
    <DropdownMenu>
      <Hint label={`Sort: ${ORDERS[order].label}`}>
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" className="size-8 shrink-0 text-muted-foreground hover:bg-surface-hover">
            <ArrowDownWideNarrowIcon className="size-4" />
          </Button>
        </DropdownMenuTrigger>
      </Hint>
      <DropdownMenuContent align="end">
        {PLAYLIST_ORDERS.map((key) => (
          <DropdownMenuItem key={key} onSelect={() => setPlaylistOrder(key)}>
            {ORDERS[key].label}
            {key === order && <CheckIcon className="ml-auto text-primary" />}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

/** Sidebar entry of a playlist; dropping a dragged song on it adds the song, and a pinned one given `pinDrag` can be dragged among the pinned. In the full column, hovering it shows a button that plays the playlist, or pauses it while it plays. */
function PlaylistLink({
  playlist,
  compact,
  pinDrag,
}: {
  playlist: Playlist
  compact: boolean
  pinDrag?: ReturnType<ReturnType<typeof usePinReorder>> | undefined
}) {
  const context = { kind: 'playlist', id: playlist.id } as const satisfies PlaybackContext
  const playingHere = useIsPlayingFrom(context)
  const { mutate: add } = useMutation(mutations.addToPlaylist())
  const [dropping, setDropping] = useState(false)
  const pinned = playlist.pin !== null

  return (
    <div className={cn('group relative', !pinned && PIN_IDLE)}>
      <PlaylistMenu playlist={playlist}>
        <Link
          to="/playlists/$playlistId"
          params={{ playlistId: playlist.id }}
          draggable={pinDrag?.draggable}
          data-pin-drop={pinDrag?.['data-pin-drop']}
          onDragStart={pinDrag?.onDragStart}
          onDragEnd={pinDrag?.onDragEnd}
          onDragOver={(event) => {
            if (!carriesTrack(event)) return pinDrag?.onDragOver(event)
            event.preventDefault()
            event.dataTransfer.dropEffect = 'copy'
            setDropping(true)
          }}
          onDragLeave={() => setDropping(false)}
          onDrop={(event) => {
            if (!carriesTrack(event)) return pinDrag?.onDrop(event)
            event.preventDefault()
            setDropping(false)
            const trackId = droppedTrack(event)
            if (trackId !== undefined) add({ id: playlist.id, name: playlist.name, trackIds: [trackId] })
          }}
          className={cn(
            'flex items-center gap-3 rounded-md p-2 transition-colors group-hover:bg-surface-hover data-[status=active]:bg-surface-hover',
            // the name stops short of the play button that hovering shows
            !compact && 'group-hover:pr-11',
            pinDropMarks.list,
            dropping && 'bg-accent ring-2 ring-primary',
          )}
        >
          {compact ? (
            <Hint label={playlist.name} side="right">
              <span className={cn('relative block rounded-md', playingHere && 'ring-2 ring-primary')}>
                <Cover src={playlist.cover} kind="playlist" pinned={pinned} className="size-12" />
                {pinned && <PinBadge size="row" />}
              </span>
            </Hint>
          ) : (
            <>
              <span className="relative shrink-0">
                <Cover src={playlist.cover} kind="playlist" pinned={pinned} className="size-12" />
                {pinned && <PinBadge size="row" />}
              </span>
              <div className="flex min-w-0 flex-col">
                <Marquee className={cn(playingHere && 'text-primary')}>{playlist.name}</Marquee>
                <span className="flex items-center gap-1 text-sm text-muted-foreground">
                  <Music2Icon className="size-3.5 shrink-0" aria-label="Songs" />
                  {playlist.trackCount.toLocaleString()}
                </span>
              </div>
              {playingHere && <Volume2Icon className="mr-2 ml-auto size-4 shrink-0 text-primary group-hover:hidden" />}
            </>
          )}
        </Link>
      </PlaylistMenu>
      {/* beside the link rather than in it, so a click plays instead of also following the link */}
      {!compact && (
        <PlayButton
          context={context}
          look="ghost"
          className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 opacity-0 transition-opacity group-hover:pointer-events-auto group-hover:opacity-100 focus-visible:pointer-events-auto focus-visible:opacity-100"
        />
      )}
    </div>
  )
}
