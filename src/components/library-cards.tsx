import { Link } from '@tanstack/react-router'
import { type ComponentProps, memo } from 'react'
import type { Album, Artist } from '@/bindings'
import { AlbumMenu } from '@/components/album-menu'
import { ArtistMenu } from '@/components/artist-menu'
import { Count, Counts } from '@/components/count'
import { Cover } from '@/components/cover'
import { MediaCard, cardLinkClass } from '@/components/media-card'
import { credit, releaseLabel } from '@/lib/format'

/** Props a card passes on to its root, such as drag handlers of a pinned card. */
type CardProps = Omit<ComponentProps<typeof MediaCard>, 'cover' | 'title' | 'subtitle' | 'context' | 'link' | 'pinned'>

/** Card of an album in a grid; memoized, as grids hold every album and a pin or an edit of one album would otherwise render all of them. */
export const AlbumCard = memo(function AlbumCard({
  album,
  showArtist = true,
  ...props
}: { album: Album; showArtist?: boolean } & CardProps) {
  // an album of nobody names its kind where the artists would be
  const owners = showArtist ? credit(album.artists) : ''
  const parts = [album.year === null ? undefined : String(album.year), owners === '' ? releaseLabel(album) : owners]
  return (
    <AlbumMenu albumId={album.id}>
      <MediaCard
        {...props}
        pinned={album.pin !== null}
        cover={<Cover src={album.cover} kind="album" pinned={album.pin !== null} />}
        title={album.title}
        subtitle={parts.filter((part) => part !== undefined).join(' • ')}
        context={{ kind: 'album', id: album.id }}
        link={
          <Link
            to="/albums/$albumId"
            params={{ albumId: album.id }}
            aria-label={album.title}
            className={cardLinkClass}
          />
        }
      />
    </AlbumMenu>
  )
})

/** Card of an artist in a grid; memoized for the same reason as `AlbumCard`. */
export const ArtistCard = memo(function ArtistCard({ artist, ...props }: { artist: Artist } & CardProps) {
  return (
    <ArtistMenu artistId={artist.id}>
      <MediaCard
        {...props}
        pinned={artist.pin !== null}
        cover={<Cover src={artist.cover} kind="artist" pinned={artist.pin !== null} />}
        title={artist.name}
        subtitle={
          <Counts>
            {artist.albumCount > 0 && <Count kind="albums" value={artist.albumCount} />}
            <Count kind="songs" value={artist.trackCount} />
          </Counts>
        }
        tightSubtitle
        context={{ kind: 'artist', id: artist.id }}
        link={
          <Link
            to="/artists/$artistId"
            params={{ artistId: artist.id }}
            aria-label={artist.name}
            className={cardLinkClass}
          />
        }
      />
    </ArtistMenu>
  )
})
