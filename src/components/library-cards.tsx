import { Link } from '@tanstack/react-router'
import type { ComponentProps } from 'react'
import type { Album, Artist } from '@/bindings'
import { AlbumMenu } from '@/components/album-menu'
import { ArtistMenu } from '@/components/artist-menu'
import { Cover } from '@/components/cover'
import { MediaCard, cardLinkClass } from '@/components/media-card'
import { plural } from '@/lib/format'

/** Props a card passes on to its root, such as drag handlers of a pinned card. */
type CardProps = Omit<ComponentProps<typeof MediaCard>, 'cover' | 'title' | 'subtitle' | 'context' | 'link' | 'pinned'>

export function AlbumCard({ album, showArtist = true, ...props }: { album: Album; showArtist?: boolean } & CardProps) {
  // an album of nobody names its kind where the artist would be
  const parts = [album.year === null ? undefined : String(album.year), (showArtist && album.artist) || 'Album']
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
}

export function ArtistCard({ artist, ...props }: { artist: Artist } & CardProps) {
  return (
    <ArtistMenu artistId={artist.id}>
      <MediaCard
        {...props}
        pinned={artist.pin !== null}
        cover={<Cover src={artist.cover} kind="artist" pinned={artist.pin !== null} />}
        title={artist.name}
        subtitle={plural(artist.trackCount, 'song')}
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
}
