import { useMutation } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { ClockIcon, PauseIcon, PlayIcon } from 'lucide-react'
import { type DragEvent, useRef, useState } from 'react'
import type { PlaybackContext, Track, TrackId } from '@/bindings'
import { ArtistMenu } from '@/components/artist-menu'
import { Cover } from '@/components/cover'
import { Equalizer } from '@/components/equalizer'
import { TrackMenu } from '@/components/track-menu'
import { startTrackDrag } from '@/lib/dnd'
import { formatClock } from '@/lib/format'
import { currentItem, player, useIsPlayingFrom, usePlayer } from '@/lib/player'
import { mutations } from '@/lib/query'
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

/** Virtualized track list of a view; playing from it makes `context` the playback context. */
export function TrackTable({ tracks, context }: { tracks: Track[]; context: PlaybackContext }) {
  const layout = context.kind === 'album' ? 'album' : 'full'
  const { listRef, virtualizer } = useMainListVirtualizer(tracks.length, ROW_HEIGHT)
  const currentId = usePlayer((state) => currentItem(state)?.track.id)
  const isPlaying = usePlayer((state) => state.isPlaying)
  const playingHere = useIsPlayingFrom(context)
  const [selected, setSelected] = useState<TrackId | null>(null)
  const reorder = usePlaylistReorder(context, tracks)

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
        <span>Title</span>
        {layout === 'full' && <span>Album</span>}
        <ClockIcon className="mr-4 size-4 justify-self-end" />
      </div>
      <div ref={listRef} className="relative" style={{ height: virtualizer.getTotalSize() }}>
        {virtualizer.getVirtualItems().map((item) => {
          const track = tracks[item.index]
          if (track === undefined) return null
          const status = statusOf(track.id)
          const dropEdge = reorder?.drop?.trackId === track.id ? reorder.drop.edge : undefined
          return (
            <TrackMenu key={track.id} track={track} context={context}>
              <div
                draggable
                onDragStart={(event) => {
                  startTrackDrag(event, track.id)
                  reorder?.start(track.id)
                }}
                onDragOver={reorder && ((event) => reorder.over(event, track.id))}
                onDrop={reorder && ((event) => reorder.land(event))}
                onDragEnd={reorder?.end}
                onClick={() => setSelected(track.id)}
                onContextMenu={() => setSelected(track.id)}
                onDoubleClick={() => player.play(context, track.id)}
                data-drop={dropEdge}
                className={cn(
                  'group absolute top-0 left-0 grid w-full items-center gap-4 rounded-md px-4 hover:bg-foreground/10',
                  'data-[drop=after]:shadow-[inset_0_-2px_0_var(--primary)] data-[drop=before]:shadow-[inset_0_2px_0_var(--primary)]',
                  columns[layout],
                  selected === track.id && 'bg-foreground/20 hover:bg-foreground/20',
                )}
                style={{
                  height: ROW_HEIGHT,
                  transform: `translateY(${item.start - virtualizer.options.scrollMargin}px)`,
                }}
              >
                <IndexCell
                  status={status}
                  position={layout === 'album' ? (track.trackNo ?? item.index + 1) : item.index + 1}
                  onPlay={() =>
                    status === 'playing' || status === 'paused' ? player.toggle() : player.play(context, track.id)
                  }
                />
                <div className="flex min-w-0 items-center gap-3">
                  {layout === 'full' && <Cover src={track.cover} kind="track" className="size-10 rounded-sm" />}
                  <div className="flex min-w-0 flex-col">
                    <span className={cn('truncate', titleColor[status])}>{track.title}</span>
                    <ArtistMenu artistId={track.artistId}>
                      <Link
                        to="/artists/$artistId"
                        params={{ artistId: track.artistId }}
                        className="w-fit max-w-full truncate text-sm text-muted-foreground group-hover:text-foreground hover:underline"
                      >
                        {track.artist}
                      </Link>
                    </ArtistMenu>
                  </div>
                </div>
                {layout === 'full' && (
                  <Link
                    to="/albums/$albumId"
                    params={{ albumId: track.albumId }}
                    className="w-fit max-w-full truncate text-sm text-muted-foreground group-hover:text-foreground hover:underline"
                  >
                    {track.album}
                  </Link>
                )}
                <span className="mr-4 justify-self-end text-sm text-muted-foreground tabular-nums">
                  {formatClock(track.durationMs)}
                </span>
              </div>
            </TrackMenu>
          )
        })}
      </div>
    </div>
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

/** Drag-and-drop reordering of playlist rows; `undefined` outside playlists. */
function usePlaylistReorder(context: PlaybackContext, tracks: Track[]) {
  const { mutate: move } = useMutation(mutations.movePlaylistTrack())
  const dragged = useRef<TrackId | null>(null)
  const [drop, setDrop] = useState<{ trackId: TrackId; edge: DropEdge } | null>(null)
  if (context.kind !== 'playlist') return undefined

  const end = () => {
    dragged.current = null
    setDrop(null)
  }

  return {
    drop,
    start: (trackId: TrackId) => {
      dragged.current = trackId
    },
    over: (event: DragEvent<HTMLElement>, trackId: TrackId) => {
      if (dragged.current === null) return
      event.preventDefault()
      event.dataTransfer.dropEffect = 'move'
      const { top, height } = event.currentTarget.getBoundingClientRect()
      const edge = event.clientY < top + height / 2 ? 'before' : 'after'
      if (drop?.trackId !== trackId || drop.edge !== edge) setDrop({ trackId, edge })
    },
    land: (event: DragEvent<HTMLElement>) => {
      event.preventDefault()
      const from = tracks.findIndex((track) => track.id === dragged.current)
      const target = tracks.findIndex((track) => track.id === drop?.trackId)
      const draggedId = dragged.current
      end()
      if (from < 0 || target < 0 || draggedId === null || drop === null) return
      const insert = drop.edge === 'before' ? target : target + 1
      // the backend inserts into the order that remains after taking the track out
      const to = insert > from ? insert - 1 : insert
      if (to !== from) move({ id: context.id, trackId: draggedId, to })
    },
    end,
  }
}
