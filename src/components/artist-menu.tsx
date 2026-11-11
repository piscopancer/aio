import { Link, useNavigate } from '@tanstack/react-router'
import { ListEndIcon, MicVocalIcon, PlayIcon } from 'lucide-react'
import { type ComponentProps, Fragment, type ReactNode } from 'react'
import type { ArtistId, ArtistName } from '@/bindings'
import { Marquee } from '@/components/marquee'
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

/** The credited artists of a song as comma-separated links to their pages, each with its own right-click menu, on one
 * line that runs as a marquee when it does not fit, as `play` says; a song credited to nobody has no such line. */
export function ArtistLinks({
  artists,
  play,
  className,
}: {
  artists: readonly ArtistName[]
  play?: ComponentProps<typeof Marquee>['play']
  className?: string
}) {
  if (artists.length === 0) return null
  return (
    <Marquee play={play} className={className}>
      <ArtistLinkList artists={artists} />
    </Marquee>
  )
}

/** The links of `ArtistLinks` alone, for a line that already runs as a marquee. */
export function ArtistLinkList({ artists }: { artists: readonly ArtistName[] }) {
  return artists.map((artist, index) => (
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
  ))
}
