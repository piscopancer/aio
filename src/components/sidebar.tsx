import { useMutation, useQuery } from '@tanstack/react-query'
import { Link } from '@tanstack/react-router'
import { LibraryIcon, PlusIcon, Volume2Icon } from 'lucide-react'
import { useState } from 'react'
import type { Playlist } from '@/bindings'
import { Cover } from '@/components/cover'
import { Hint } from '@/components/hint'
import { CreatePlaylistDialog } from '@/components/playlist-dialogs'
import { PlaylistMenu } from '@/components/playlist-menu'
import { Button } from '@/components/ui/button'
import { carriesTrack, droppedTrack } from '@/lib/dnd'
import { plural } from '@/lib/format'
import { toggleSidebar, useLayout } from '@/lib/layout'
import { useIsPlayingFrom } from '@/lib/player'
import { mutations, queries } from '@/lib/query'
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

  return (
    <section className="flex min-h-0 flex-1 flex-col rounded-lg bg-surface">
      <header
        className={cn('flex items-center gap-2 pt-4 pb-2 text-muted-foreground', compact ? 'flex-col px-2' : 'px-4')}
      >
        <Hint label={`${compact ? 'Expand' : 'Collapse'} playlists (Ctrl+B)`} side="right">
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
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {playlists.length === 0 && !compact ? (
          <p className="px-4 py-3 text-sm text-muted-foreground">
            Create a playlist with the + button, then drag songs onto it.
          </p>
        ) : (
          playlists.map((playlist) => <PlaylistLink key={playlist.id} playlist={playlist} compact={compact} />)
        )}
      </div>
    </section>
  )
}

/** Sidebar entry of a playlist; dropping a dragged song on it adds the song. */
function PlaylistLink({ playlist, compact }: { playlist: Playlist; compact: boolean }) {
  const playingHere = useIsPlayingFrom({ kind: 'playlist', id: playlist.id })
  const { mutate: add } = useMutation(mutations.addToPlaylist())
  const [dropping, setDropping] = useState(false)

  return (
    <PlaylistMenu playlist={playlist}>
      <Link
        to="/playlists/$playlistId"
        params={{ playlistId: playlist.id }}
        onDragOver={(event) => {
          if (!carriesTrack(event)) return
          event.preventDefault()
          event.dataTransfer.dropEffect = 'copy'
          setDropping(true)
        }}
        onDragLeave={() => setDropping(false)}
        onDrop={(event) => {
          event.preventDefault()
          setDropping(false)
          const trackId = droppedTrack(event)
          if (trackId !== undefined) add({ id: playlist.id, name: playlist.name, trackIds: [trackId] })
        }}
        className={cn(
          'flex items-center gap-3 rounded-md p-2 transition-colors hover:bg-surface-hover data-[status=active]:bg-surface-hover',
          dropping && 'bg-accent ring-2 ring-primary',
        )}
      >
        {compact ? (
          <Hint label={playlist.name} side="right">
            <span className={cn('block rounded-md', playingHere && 'ring-2 ring-primary')}>
              <Cover src={playlist.cover} kind="playlist" className="size-12" />
            </span>
          </Hint>
        ) : (
          <>
            <Cover src={playlist.cover} kind="playlist" className="size-12" />
            <div className="flex min-w-0 flex-col">
              <span className={cn('truncate', playingHere && 'text-primary')}>{playlist.name}</span>
              <span className="truncate text-sm text-muted-foreground">{plural(playlist.trackCount, 'song')}</span>
            </div>
            {playingHere && <Volume2Icon className="ml-auto size-4 shrink-0 text-primary" />}
          </>
        )}
      </Link>
    </PlaylistMenu>
  )
}
