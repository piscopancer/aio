import { ListEndIcon, PlayIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import type { AlbumId } from '@/bindings'
import { PinMenuItem } from '@/components/pin-controls'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'
import { player } from '@/lib/player'
import { queries, queryClient } from '@/lib/query'

/** Right-click menu of an album card: play it, queue it, pin it. */
export function AlbumMenu({ albumId, children }: { albumId: AlbumId; children: ReactNode }) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild>{children}</ContextMenuTrigger>
      <ContextMenuContent>
        <ContextMenuItem onSelect={() => player.play({ kind: 'album', id: albumId })}>
          <PlayIcon />
          Play album
        </ContextMenuItem>
        <ContextMenuItem
          onSelect={async () => {
            const { tracks } = await queryClient.fetchQuery(queries.album(albumId))
            player.enqueue(tracks.map((track) => track.id))
          }}
        >
          <ListEndIcon />
          Add to queue
        </ContextMenuItem>
        <ContextMenuSeparator />
        <PinMenuItem target={{ kind: 'album', id: albumId }} />
      </ContextMenuContent>
    </ContextMenu>
  )
}
