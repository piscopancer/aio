import { Link } from '@tanstack/react-router'
import type { Album, Artist } from '@/bindings'
import { Cover } from '@/components/cover'
import { MediaCard, cardLinkClass } from '@/components/media-card'
import { plural } from '@/lib/format'

export function AlbumCard({ album, showArtist = true }: { album: Album; showArtist?: boolean }) {
  const parts = [album.year === null ? undefined : String(album.year), showArtist ? album.artist : 'Album']
  return (
    <MediaCard
      cover={<Cover src={album.cover} kind="album" />}
      title={album.title}
      subtitle={parts.filter((part) => part !== undefined).join(' • ')}
      context={{ kind: 'album', id: album.id }}
      link={
        <Link to="/albums/$albumId" params={{ albumId: album.id }} aria-label={album.title} className={cardLinkClass} />
      }
    />
  )
}

export function ArtistCard({ artist }: { artist: Artist }) {
  return (
    <MediaCard
      cover={<Cover src={artist.cover} kind="artist" />}
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
  )
}
