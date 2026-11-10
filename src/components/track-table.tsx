import { useMutation } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import {
  ChevronDownIcon,
  ChevronUpIcon,
  ClockIcon,
  EyeOffIcon,
  type LucideIcon,
  PauseIcon,
  PlayIcon,
  TriangleAlertIcon,
} from 'lucide-react'
import { type DragEvent, type ReactNode, memo, useEffect, useMemo, useRef, useState } from 'react'
import type { PlaybackContext, Presence, Track, TrackId } from '@/bindings'
import { ArtistLinks } from '@/components/artist-menu'
import { Cover } from '@/components/cover'
import { Hint } from '@/components/hint'
import { Marquee } from '@/components/marquee'
import { Equalizer } from '@/components/equalizer'
import { TrackMenu } from '@/components/track-menu'
import { startTrackDrag } from '@/lib/dnd'
import { formatClock } from '@/lib/format'
import { useLocateRequest } from '@/lib/locate'
import { contextKey, currentItem, player, showOrder, useIsPlayingFrom, usePlayer } from '@/lib/player'
import { mutations } from '@/lib/query'
import { type TrackSort, type TrackSortKey, cycleTrackSort, sortTracks, useTrackSorts } from '@/lib/track-sort'
import { cn } from '@/lib/utils'
import { useMainListVirtualizer } from '@/lib/virtual'

const ROW_HEIGHT = 56

/** Album pages drop the album column and the per-row artwork, every other view shows both. */
const columns = {
  full: 'grid-cols-[2.5rem_minmax(0,6fr)_minmax(0,4fr)_4.5rem]',
  album: 'grid-cols-[2.5rem_minmax(0,1fr)_4.5rem]',
} as const

/** How a row relates to the player: `elsewhere` means the same track plays from another view. */
type RowStatus = 'idle' | 'playing' | 'paused' | 'elsewhere'

const titleColor = {
  idle: 'text-foreground',
  playing: 'text-primary',
  paused: 'text-primary',
  elsewhere: 'text-primary-dim',
} as const satisfies Record<RowStatus, string>

type DropEdge = 'before' | 'after'

/** Virtualized track list of a view, sortable by its column headers; playing from it makes `context` the playback context, in the order shown. */
export function TrackTable({
  tracks: pageTracks,
  context: pageContext,
}: {
  tracks: Track[]
  context: PlaybackContext
}) {
  const key = contextKey(pageContext)
  // pages build a fresh context object on every render, which would defeat the memoized rows
  const context = useMemo(() => pageContext, [key])
  const layout = context.kind === 'album' ? 'album' : 'full'
  const sort = useTrackSorts((sorts) => sorts[context.kind])
  const tracks = useMemo(() => sortTracks(pageTracks, sort), [pageTracks, sort])
  // a sorted table hands its order to the player for as long as it is shown
  useEffect(() => {
    if (sort === undefined) return undefined
    return showOrder(
      context,
      tracks.map((track) => track.id),
    )
  }, [context, sort, tracks])
  const { listRef, items, totalSize, scrollMargin, centerRow } = useMainListVirtualizer(tracks.length, ROW_HEIGHT)
  const currentId = usePlayer((state) => currentItem(state)?.track.id)
  const isPlaying = usePlayer((state) => state.isPlaying)
  const playingHere = useIsPlayingFrom(context)
  const [selected, setSelected] = useState<TrackId | null>(null)
  const locate = useLocateRequest((state) => state.request)
  // a passive effect, so the song is centred after the main view has put back the scroll of the page it opened
  useEffect(() => {
    if (locate?.key !== key) return
    useLocateRequest.setState({ request: null })
    const index = tracks.findIndex((track) => track.id === locate.trackId)
    if (index < 0) return
    setSelected(locate.trackId)
    centerRow(index)
  }, [locate, key, tracks, centerRow])
  const playlistReorder = usePlaylistReorder(context, tracks)
  // dragging a song to a new place only makes sense in the playlist's own order
  const reorder = sort === undefined ? playlistReorder : undefined

  const statusOf = (id: TrackId): RowStatus => {
    if (id !== currentId) return 'idle'
    if (!playingHere) return 'elsewhere'
    return isPlaying ? 'playing' : 'paused'
  }

  return (
    <div className="px-6 pb-10">
      <div
        className={cn(
          'sticky top-0 z-10 -mx-6 mb-2 grid h-9 items-center gap-4 border-b bg-surface px-10 text-sm text-muted-foreground',
          columns[layout],
        )}
      >
        <span className="text-center">#</span>
        <SortHeader kind={context.kind} column="title" sort={sort} label="Title">
          Title
        </SortHeader>
        {layout === 'full' && (
          <SortHeader kind={context.kind} column="album" sort={sort} label="Album">
            Album
          </SortHeader>
        )}
        <SortHeader kind={context.kind} column="duration" sort={sort} label="Duration" align="end" className="pr-6">
          <ClockIcon className="size-4" />
        </SortHeader>
      </div>
      <div ref={listRef} className="relative" style={{ height: totalSize }}>
        {items.map((item) => {
          const track = tracks[item.index]
          if (track === undefined) return null
          return (
            // scrolling only moves this wrapper; the memoized row inside renders again only when its own data changes
            <div
              key={track.id}
              className="absolute top-0 left-0 w-full"
              style={{
                height: ROW_HEIGHT,
                transform: `translateY(${item.start - scrollMargin}px)`,
              }}
            >
              <TrackRow
                track={track}
                context={context}
                layout={layout}
                position={layout === 'album' ? (track.trackNo ?? item.index + 1) : item.index + 1}
                status={statusOf(track.id)}
                selected={selected === track.id}
                dropEdge={reorder?.drop?.trackId === track.id ? reorder.drop.edge : undefined}
                reorder={reorder?.handlers}
                onSelect={setSelected}
              />
            </div>
          )
        })}
      </div>
    </div>
  )
}

/** One track of the table with its menus and links, the costly part of a frame while scrolling. */
const TrackRow = memo(function TrackRow({
  track,
  context,
  layout,
  position,
  status,
  selected,
  dropEdge,
  reorder,
  onSelect,
}: {
  track: Track
  context: PlaybackContext
  layout: keyof typeof columns
  position: number
  status: RowStatus
  selected: boolean
  dropEdge: DropEdge | undefined
  reorder: ReorderHandlers | undefined
  onSelect: (id: TrackId) => void
}) {
  return (
    <TrackMenu track={track} context={context}>
      <div
        draggable
        onDragStart={(event) => {
          startTrackDrag(event, track.id)
          reorder?.start(track.id)
        }}
        onDragOver={reorder && ((event) => reorder.over(event, track.id))}
        onDrop={reorder?.land}
        onDragEnd={reorder?.end}
        onClick={() => onSelect(track.id)}
        onContextMenu={() => onSelect(track.id)}
        onDoubleClick={() => track.presence === 'present' && player.play(context, track.id)}
        data-drop={dropEdge}
        className={cn(
          'group grid size-full items-center gap-4 rounded-md px-4 hover:bg-foreground/10',
          'data-[drop=after]:shadow-[inset_0_-2px_0_var(--primary)] data-[drop=before]:shadow-[inset_0_2px_0_var(--primary)]',
          columns[layout],
          selected && 'bg-foreground/20 hover:bg-foreground/20',
          track.presence !== 'present' && 'text-muted-foreground *:not-first:opacity-50',
        )}
      >
        {track.presence === 'present' ? (
          <IndexCell
            status={status}
            position={position}
            onPlay={() =>
              status === 'playing' || status === 'paused' ? player.toggle() : player.play(context, track.id)
            }
          />
        ) : (
          <UnplayableMark presence={track.presence} />
        )}
        <div className="flex min-w-0 items-center gap-3">
          {layout === 'full' && <Cover src={track.cover} kind="track" className="size-10 rounded-sm" />}
          <div className="flex min-w-0 flex-col">
            <Marquee className={titleColor[status]}>{track.title}</Marquee>
            <ArtistLinks
              artists={track.artists}
              className="text-sm text-muted-foreground group-hover:text-foreground"
            />
          </div>
        </div>
        {layout === 'full' &&
          (track.albumId === null ? (
            // a song on no album leaves the cell empty, keeping the grid's column
            <span />
          ) : (
            <Marquee className="text-sm text-muted-foreground group-hover:text-foreground">
              <Link to="/albums/$albumId" params={{ albumId: track.albumId }} className="hover:underline">
                {track.album}
              </Link>
            </Marquee>
          ))}
        <span className="mr-4 justify-self-end text-sm text-muted-foreground tabular-nums">
          {formatClock(track.durationMs)}
        </span>
      </div>
    </TrackMenu>
  )
})

/** Column header that cycles the table through descending, ascending and the page's own order, with a chevron while it sorts; a header aligned to the `end` puts the chevron before its content, so the content stays in line with the cells below. */
function SortHeader({
  kind,
  column,
  sort,
  label,
  align = 'start',
  className,
  children,
}: {
  kind: PlaybackContext['kind']
  column: TrackSortKey
  sort: TrackSort | undefined
  label: string
  align?: 'start' | 'end'
  className?: string
  children: ReactNode
}) {
  const direction = sort?.key === column ? sort.direction : undefined
  const Chevron = direction === 'desc' ? ChevronDownIcon : ChevronUpIcon
  return (
    <button
      type="button"
      aria-label={`Sort by ${label.toLowerCase()}`}
      onClick={() => cycleTrackSort(kind, column)}
      className={cn(
        // the whole cell and a little of the gaps beside it take the click
        '-mx-2 flex cursor-pointer items-center gap-1.5 self-stretch px-2 hover:text-foreground',
        direction !== undefined && 'text-foreground',
        // reversed, the row starts at the right edge, so the content keeps it and the chevron goes to its left
        align === 'end' && 'flex-row-reverse',
        className,
      )}
    >
      {children}
      {direction !== undefined && <Chevron className="size-4 text-primary" />}
    </button>
  )
}

/** Why a song a playlist still lists cannot be played, told where its play button would be. */
const unplayable = {
  hidden: { icon: EyeOffIcon, hint: 'Its folder is hidden from the library' },
  missing: { icon: TriangleAlertIcon, hint: 'The file is no longer in your library' },
} as const satisfies Record<Exclude<Presence, 'present'>, { icon: LucideIcon; hint: string }>

function UnplayableMark({ presence }: { presence: keyof typeof unplayable }) {
  const { icon: Icon, hint } = unplayable[presence]
  return (
    <Hint label={hint}>
      <Icon className="size-4 justify-self-center text-muted-foreground" />
    </Hint>
  )
}

function IndexCell({ status, position, onPlay }: { status: RowStatus; position: number; onPlay: () => void }) {
  const Icon = status === 'playing' ? PauseIcon : PlayIcon
  return (
    <div className="flex justify-center">
      <span
        className={cn(
          'tabular-nums group-hover:hidden',
          status === 'idle' ? 'text-muted-foreground' : titleColor[status],
        )}
      >
        {status === 'playing' ? <Equalizer /> : position}
      </span>
      <button
        type="button"
        aria-label={status === 'playing' ? 'Pause' : 'Play'}
        onClick={(event) => {
          event.stopPropagation()
          onPlay()
        }}
        className="hidden cursor-pointer group-hover:block"
      >
        <Icon className="size-4 fill-current" />
      </button>
    </div>
  )
}

type ReorderHandlers = {
  start: (trackId: TrackId) => void
  over: (event: DragEvent<HTMLElement>, trackId: TrackId) => void
  land: (event: DragEvent<HTMLElement>) => void
  end: () => void
}

/** Drag-and-drop reordering of playlist rows, `undefined` outside playlists; the handlers never change, so rows stay memoized while a drag moves the drop mark. */
function usePlaylistReorder(context: PlaybackContext, tracks: Track[]) {
  const { mutate: move } = useMutation(mutations.movePlaylistTrack())
  const dragged = useRef<TrackId | null>(null)
  const [drop, setDrop] = useState<{ trackId: TrackId; edge: DropEdge } | null>(null)
  // the handlers read the latest values through this ref instead of closing over one render
  const latest = useRef({ context, tracks, drop, move })
  latest.current = { context, tracks, drop, move }

  const handlers = useMemo<ReorderHandlers>(() => {
    const end = () => {
      dragged.current = null
      setDrop(null)
    }
    return {
      start: (trackId) => {
        dragged.current = trackId
      },
      over: (event, trackId) => {
        if (dragged.current === null) return
        event.preventDefault()
        event.dataTransfer.dropEffect = 'move'
        const { top, height } = event.currentTarget.getBoundingClientRect()
        const edge = event.clientY < top + height / 2 ? 'before' : 'after'
        const current = latest.current.drop
        if (current?.trackId !== trackId || current.edge !== edge) setDrop({ trackId, edge })
      },
      land: (event) => {
        event.preventDefault()
        const { context, tracks, drop, move } = latest.current
        const draggedId = dragged.current
        const from = tracks.findIndex((track) => track.id === draggedId)
        const target = tracks.findIndex((track) => track.id === drop?.trackId)
        end()
        if (context.kind !== 'playlist' || from < 0 || target < 0 || draggedId === null || drop === null) return
        const insert = drop.edge === 'before' ? target : target + 1
        // the backend inserts into the order that remains after taking the track out
        const to = insert > from ? insert - 1 : insert
        if (to !== from) move({ id: context.id, trackId: draggedId, to })
      },
      end,
    }
  }, [])

  if (context.kind !== 'playlist') return undefined
  return { drop, handlers }
}
