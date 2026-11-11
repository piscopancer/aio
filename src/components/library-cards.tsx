import { Link } from '@tanstack/react-router'
import { type ComponentProps, memo } from 'react'
import type { Album, Artist, PlaybackContext, Playlist, Track } from '@/bindings'
import { AlbumMenu } from '@/components/album-menu'
import { ArtistLinkList, ArtistMenu } from '@/components/artist-menu'
import { Count, Counts } from '@/components/count'
import { Cover } from '@/components/cover'
import { MediaCard, cardLinkClass } from '@/components/media-card'
import { PlaylistMenu } from '@/components/playlist-menu'
import { TrackMenu } from '@/components/track-menu'
import { credit, releaseLabel } from '@/lib/format'
import { player } from '@/lib/player'

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

/** Card of a playlist in a grid, with the same menu as its entry in the sidebar; memoized for the same reason as `AlbumCard`. */
export const PlaylistCard = memo(function PlaylistCard({ playlist, ...props }: { playlist: Playlist } & CardProps) {
  return (
    <PlaylistMenu playlist={playlist}>
      <MediaCard
        {...props}
        pinned={playlist.pin !== null}
        cover={<Cover src={playlist.cover} kind="playlist" pinned={playlist.pin !== null} />}
        title={playlist.name}
        subtitle={
          <Counts>
            <Count kind="songs" value={playlist.trackCount} />
          </Counts>
        }
        context={{ kind: 'playlist', id: playlist.id }}
        link={
          <Link
            to="/playlists/$playlistId"
            params={{ playlistId: playlist.id }}
            aria-label={playlist.name}
            className={cardLinkClass}
          />
        }
      />
    </PlaylistMenu>
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

/** Card of a song that plays it from `context`, the rest of the context following; memoized for the same reason as
 * `AlbumCard`. */
export const TrackCard = memo(function TrackCard({
  track,
  context,
  ...props
}: { track: Track; context: PlaybackContext } & CardProps) {
  return (
    <TrackMenu track={track} context={context}>
      <MediaCard
        {...props}
        cover={<Cover src={track.cover} kind="track" />}
        title={track.title}
        subtitle={
          // above the card's play layer, so each name opens its artist
          <span className="relative z-10">
            <ArtistLinkList artists={track.artists} />
          </span>
        }
        context={context}
        trackId={track.id}
        link={
          <button
            type="button"
            aria-label={track.title}
            className={cardLinkClass}
            onClick={() => player.playOrToggle(context, track.id)}
          />
        }
      />
    </TrackMenu>
  )
})
