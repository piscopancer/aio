import { useMutation, useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import {
  ArrowDownWideNarrowIcon,
  LibraryIcon,
  PanelLeftCloseIcon,
  PanelLeftOpenIcon,
  PlusIcon,
  Volume2Icon,
} from 'lucide-react'
import { useRef, useState } from 'react'
import type { PlaybackContext, Playlist } from '@/bindings'
import { Count, Counts } from '@/components/count'
import { Cover } from '@/components/cover'
import { Hint } from '@/components/hint'
import { tabClass } from '@/components/library-tabs'
import { Marquee } from '@/components/marquee'
import { CreatePlaylistDialog } from '@/components/playlist-dialogs'
import { PinBadge } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { PlaylistMenu } from '@/components/playlist-menu'
import { Button } from '@/components/ui/button'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { Shortcut } from '@/components/ui/shortcut'
import { carriesTrack, droppedTrack } from '@/lib/dnd'
import {
  CREATE_PLAYLIST_KEYS,
  PLAYLIST_ORDERS,
  type PlaylistOrder,
  SIDEBAR_KEYS,
  setCreatingPlaylist,
  setPlaylistOrder,
  toggleSidebar,
  useLayout,
} from '@/lib/layout'
import { player, useIsPlayingFrom } from '@/lib/player'
import { mutations, queries } from '@/lib/query'
import { PIN_IDLE, pinDropMarks, usePinReorder } from '@/lib/pins'
import { ORDERS, inListOrder } from '@/lib/playlists'
import { cn } from '@/lib/utils'

/** The left column, given over to the playlists. */
export function Sidebar() {
  return (
    <aside className={cn('flex shrink-0 flex-col', useLayout((state) => state.sidebarCompact) ? 'w-20' : 'w-72')}>
      <Library />
    </aside>
  )
}

/** Size of the header's icon buttons in the narrow column: the pill of the width switch above them, 18px either side of a 16px icon. */
const narrowHeaderButton = 'h-10 w-13'

/** What switching the column does from its current width, in the hint of the column's button and in its menu. */
const toggleLabel = (compact: boolean) => `${compact ? 'Expand' : 'Collapse'} playlists`

/** The playlists column; right-clicking its empty space opens a menu that creates a playlist or switches the width. */
function Library() {
  const { data: playlists = [] } = useQuery(queries.playlists())
  const creating = useLayout((state) => state.creatingPlaylist)
  // while the menu is open it holds the focus, so a dialog opened from it waits until it has closed
  const createOnClose = useRef(false)
  const compact = useLayout((state) => state.sidebarCompact)
  const order = useLayout((state) => state.playlistOrder)

  return (
    <>
      <ContextMenu>
        <ContextMenuTrigger asChild>
          <section className="flex min-h-0 flex-1 flex-col rounded-lg bg-surface">
            <header
              className={cn(
                // the same padding as the library tabs, so both rows line up across the window, and a line under it apart from the list
                'flex items-center border-b py-3 text-muted-foreground',
                // 14px in the wide column puts the icon of the width switch where centring puts it in the narrow one, so switching does not move it
                compact ? 'flex-col px-2' : 'gap-2 px-3.5',
              )}
            >
              <Hint label={toggleLabel(compact)} keys={SIDEBAR_KEYS} side="right">
                {/* a library tab chip at the size of a tab on its track, as the Folders tab is, without the track itself */}
                <button
                  type="button"
                  className={cn(tabClass, 'cursor-pointer hover:bg-surface-hover', 'h-10 px-4.5')}
                  onClick={toggleSidebar}
                >
                  <LibraryIcon />
                  {!compact && 'Playlists'}
                </button>
              </Hint>
              {playlists.length > 0 && <OrderMenu order={order} compact={compact} />}
              <Hint label="Create playlist" keys={CREATE_PLAYLIST_KEYS} side="right">
                <Button
                  variant="ghost"
                  size="icon"
                  // the order picker, when there, takes the free space before it
                  className={cn(
                    'hover:bg-surface-hover',
                    compact ? narrowHeaderButton : playlists.length === 0 && 'ml-auto',
                  )}
                  onClick={() => setCreatingPlaylist(true)}
                >
                  <PlusIcon />
                </Button>
              </Hint>
            </header>
            <div className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto p-2">
              {playlists.length === 0 && !compact ? (
                <p className="px-4 py-3 text-sm text-muted-foreground">
                  Create a playlist with the + button, then drag songs onto it.
                </p>
              ) : (
                <PlaylistRows playlists={playlists} order={order} compact={compact} reorderable />
              )}
            </div>
          </section>
        </ContextMenuTrigger>
        <ContextMenuContent
          onCloseAutoFocus={(event) => {
            if (!createOnClose.current) return
            createOnClose.current = false
            // the dialog takes the focus instead of the column the menu would hand it back to
            event.preventDefault()
            setCreatingPlaylist(true)
          }}
        >
          <ContextMenuItem
            onSelect={() => {
              createOnClose.current = true
            }}
          >
            <PlusIcon />
            Create playlist
            <Shortcut keys={CREATE_PLAYLIST_KEYS} className="ml-auto" />
          </ContextMenuItem>
          <ContextMenuItem onSelect={toggleSidebar}>
            {compact ? <PanelLeftOpenIcon /> : <PanelLeftCloseIcon />}
            {toggleLabel(compact)}
            <Shortcut keys={SIDEBAR_KEYS} className="ml-auto" />
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      {/* outside the menu's trigger, so a right click in the dialog does not open the column's menu */}
      <CreatePlaylistDialog open={creating} onOpenChange={setCreatingPlaylist} />
    </>
  )
}

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
  const { pinned, rest } = inListOrder(playlists, order)
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
      {rest.map((playlist) => (
        <PlaylistLink key={playlist.id} playlist={playlist} compact={compact} />
      ))}
    </>
  )
}

/** Icon button beside the create button that opens the list of orders, the current one lit in the list and named in its hint; in the narrow column both open to the right. */
function OrderMenu({ order, compact }: { order: PlaylistOrder; compact: boolean }) {
  return (
    <DropdownMenu>
      <Hint label={`Sort: ${ORDERS[order].label}`} side={compact ? 'right' : undefined}>
        <DropdownMenuTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className={cn('hover:bg-surface-hover', compact ? narrowHeaderButton : 'ml-auto')}
          >
            <ArrowDownWideNarrowIcon />
          </Button>
        </DropdownMenuTrigger>
      </Hint>
      <DropdownMenuContent side={compact ? 'right' : 'bottom'} align={compact ? 'start' : 'end'}>
        {PLAYLIST_ORDERS.map((key) => {
          const { label, icon: Icon } = ORDERS[key]
          return (
            // the order in use is lit like an active tab rather than ticked, and the others hover dimmer so the two never look alike
            <DropdownMenuItem
              key={key}
              className={key === order ? 'bg-accent' : 'focus:bg-foreground/5'}
              onSelect={() => setPlaylistOrder(key)}
            >
              <Icon className={cn(key === order && 'text-foreground')} />
              {label}
            </DropdownMenuItem>
          )
        })}
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
          // a double click plays the playlist, or pauses it when it is playing, as its play button does
          onDoubleClick={playlist.trackCount > 0 ? () => player.playOrToggle(context) : undefined}
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
            // the cover's own radius plus the padding around it, so the narrow tile's corners run parallel to the cover's
            compact && 'rounded-[calc(var(--radius-md)+--spacing(2))]',
            // the narrow list has no name to colour, so the tile of the playlist playing now is tinted instead
            compact && playingHere && 'bg-primary/20 group-hover:bg-primary/30 data-[status=active]:bg-primary/30',
            pinDropMarks.list,
            dropping && 'bg-accent ring-1 ring-primary',
          )}
        >
          {compact ? (
            <Hint label={playlist.name} side="right">
              <span className="relative block rounded-md">
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
                <Counts>
                  <Count kind="songs" value={playlist.trackCount} />
                </Counts>
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
