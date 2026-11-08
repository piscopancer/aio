import { useMutation, useQuery } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { Disc3Icon, ListEndIcon, ListPlusIcon, MicVocalIcon, PlayIcon, PlusIcon, Trash2Icon } from 'lucide-react'
import type { ReactNode } from 'react'
import type { PlaybackContext, Track } from '@/bindings'
import { Cover } from '@/components/cover'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuSub,
  ContextMenuSubContent,
  ContextMenuSubTrigger,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { player } from '@/lib/player'
import { useCreatePlaylist } from '@/lib/playlists'
import { mutations, queries } from '@/lib/query'

type TrackMenuProps = { track: Track; context: PlaybackContext }

/** Right-click menu of a track row. */
export function TrackMenu({ children, ...props }: TrackMenuProps & { children: ReactNode }) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <TrackMenuItems {...props} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

/** Mounted only while the menu is open, so closed rows run no queries. */
function TrackMenuItems({ track, context }: TrackMenuProps) {
  const navigate = useNavigate()
  const { data: playlists = [] } = useQuery(queries.playlists())
  const addToPlaylist = useMutation(mutations.addToPlaylist())
  const removeFromPlaylist = useMutation(mutations.removeFromPlaylist())
  const createPlaylist = useCreatePlaylist()
  const trackIds = [track.id]

  return (
    <>
      <ContextMenuItem onSelect={() => player.play(context, track.id)}>
        <PlayIcon />
        Play
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => player.enqueue(trackIds)}>
        <ListEndIcon />
        Add to queue
      </ContextMenuItem>
      <ContextMenuSeparator />
      <ContextMenuSub>
        <ContextMenuSubTrigger>
          <ListPlusIcon />
          Add to playlist
        </ContextMenuSubTrigger>
        <ContextMenuSubContent className="max-h-80">
          <ContextMenuItem
            onSelect={async () => {
              const { id, name } = await createPlaylist()
              addToPlaylist.mutate({ id, name, trackIds })
            }}
          >
            <PlusIcon />
            New playlist
          </ContextMenuItem>
          {playlists.length > 0 && <ContextMenuSeparator />}
          {playlists.map(({ id, name, cover }) => (
            <ContextMenuItem key={id} onSelect={() => addToPlaylist.mutate({ id, name, trackIds })}>
              <Cover src={cover} kind="playlist" className="-my-1 size-7 rounded-sm" />
              <span className="max-w-64 truncate">{name}</span>
            </ContextMenuItem>
          ))}
        </ContextMenuSubContent>
      </ContextMenuSub>
      {context.kind === 'playlist' && (
        <ContextMenuItem onSelect={() => removeFromPlaylist.mutate({ id: context.id, trackIds })}>
          <Trash2Icon />
          Remove from this playlist
        </ContextMenuItem>
      )}
      <ContextMenuSeparator />
      <ContextMenuItem onSelect={() => navigate({ to: '/artists/$artistId', params: { artistId: track.artistId } })}>
        <MicVocalIcon />
        Go to artist
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => navigate({ to: '/albums/$albumId', params: { albumId: track.albumId } })}>
        <Disc3Icon />
        Go to album
      </ContextMenuItem>
    </>
  )
}
