import { Link, useNavigate } from '@tanstack/react-router'
import { ListEndIcon, MicVocalIcon, PlayIcon } from 'lucide-react'
import { Fragment, type ReactNode } from 'react'
import type { ArtistId, ArtistName } from '@/bindings'
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
import { cn } from '@/lib/utils'

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
      <ContextMenuSeparator />
      <PinMenuItem target={{ kind: 'artist', id: artistId }} />
    </>
  )
}

/** The credited artists of a song as comma-separated links to their pages, each with its own right-click menu. */
export function ArtistLinks({ artists, className }: { artists: readonly ArtistName[]; className?: string }) {
  return (
    <span className={cn('truncate', className)}>
      {artists.map((artist, index) => (
        <Fragment key={artist.id}>
          {index > 0 && ', '}
          <ArtistMenu artistId={artist.id}>
            <Link
              to="/artists/$artistId"
              params={{ artistId: artist.id }}
              className="hover:text-foreground hover:underline"
            >
              {artist.name}
            </Link>
          </ArtistMenu>
        </Fragment>
      ))}
    </span>
  )
}
