import { ListEndIcon, PencilIcon, PlayIcon, Trash2Icon } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import type { Playlist } from '@/bindings'
import { type PlaylistDialog, PlaylistDialogs } from '@/components/playlist-dialogs'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { player } from '@/lib/player'
import { queries, queryClient } from '@/lib/query'

/** Right-click menu of a playlist entry, with its rename and delete dialogs. */
export function PlaylistMenu({ playlist, children }: { playlist: Playlist; children: ReactNode }) {
  const [dialog, setDialog] = useState<PlaylistDialog>(null)
  return (
    <>
      <ContextMenu>
        <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
        <ContextMenuContent>
          <ContextMenuItem
            disabled={playlist.trackCount === 0}
            onSelect={() => player.play({ kind: 'playlist', id: playlist.id })}
          >
            <PlayIcon />
            Play
          </ContextMenuItem>
          <ContextMenuItem
            disabled={playlist.trackCount === 0}
            onSelect={async () => {
              const { tracks } = await queryClient.fetchQuery(queries.playlist(playlist.id))
              player.enqueue(tracks.map((track) => track.id))
            }}
          >
            <ListEndIcon />
            Add to queue
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem onSelect={() => setDialog('edit')}>
            <PencilIcon />
            Edit details
          </ContextMenuItem>
          <ContextMenuItem variant="destructive" onSelect={() => setDialog('delete')}>
            <Trash2Icon />
            Delete
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      <PlaylistDialogs playlist={playlist} open={dialog} onClose={() => setDialog(null)} />
    </>
  )
}
