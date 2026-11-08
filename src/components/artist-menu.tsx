import { useNavigate } from '@tanstack/react-router'
import { ListEndIcon, MicVocalIcon, PlayIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import type { ArtistId } from '@/bindings'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu'
import { player } from '@/lib/player'
import { queries, queryClient } from '@/lib/query'

/** Right-click menu of an artist name; it wins over the menu of the row the name sits in. */
export function ArtistMenu({ artistId, children }: { artistId: ArtistId; children: ReactNode }) {
  return (
    <ContextMenu>
      <ContextMenuTrigger asChild onContextMenu={(event) => event.stopPropagation()}>
        {children}
      </ContextMenuTrigger>
      <ContextMenuContent>
        <ArtistMenuItems artistId={artistId} />
      </ContextMenuContent>
    </ContextMenu>
  )
}

function ArtistMenuItems({ artistId }: { artistId: ArtistId }) {
  const navigate = useNavigate()
  return (
    <>
      <ContextMenuItem onSelect={() => player.play({ kind: 'artist', id: artistId })}>
        <PlayIcon />
        Play artist
      </ContextMenuItem>
      <ContextMenuItem
        onSelect={async () => {
          const { tracks } = await queryClient.fetchQuery(queries.artist(artistId))
          player.enqueue(tracks.map((track) => track.id))
        }}
      >
        <ListEndIcon />
        Add to queue
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => navigate({ to: '/artists/$artistId', params: { artistId } })}>
        <MicVocalIcon />
        Go to artist
      </ContextMenuItem>
    </>
  )
}
