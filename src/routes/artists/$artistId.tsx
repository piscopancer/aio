import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import type { Album } from '@/bindings'
import { Cover } from '@/components/cover'
import { AlbumCard } from '@/components/library-cards'
import { MediaGrid } from '@/components/media-card'
import { MetaStat, MetaStats, PageHeader } from '@/components/page-header'
import { PinButton } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { TrackTable } from '@/components/track-table'
import { totalDuration } from '@/lib/format'
import { idParam } from '@/lib/params'
import { queries } from '@/lib/query'

export const Route = createFileRoute('/artists/$artistId')({
  params: idParam('artistId'),
  loader: ({ context, params }) => context.queryClient.ensureQueryData(queries.artist(params.artistId)),
  component: ArtistPage,
})

function ArtistPage() {
  const { artistId } = Route.useParams()
  const {
    data: { artist, albums, appearsOn, tracks },
  } = useSuspenseQuery(queries.artist(artistId))
  const context = { kind: 'artist', id: artist.id } as const
  // an album whose files do not say what it is counts as an album
  const short = (album: Album) => album.releaseType === 'single' || album.releaseType === 'ep'

  return (
    <>
      <PageHeader
        eyebrow="Artist"
        title={artist.name}
        art={artist.cover}
        cover={<Cover src={artist.cover} kind="artist" />}
        meta={
          <>
            <MetaStats>
              <MetaStat kind="albums" value={artist.albumCount} />
              <MetaStat kind="songs" value={artist.trackCount} />
              {tracks.length > 0 && <MetaStat kind="length" value={totalDuration(tracks)} />}
            </MetaStats>
          </>
        }
      >
        <PlayButton context={context} disabled={tracks.length === 0} />
        <PinButton target={{ kind: 'artist', id: artist.id }} />
      </PageHeader>
      {/* an album artist may have no songs of their own, only the albums below */}
      {tracks.length > 0 && (
        <section className="pb-6">
          <TrackTable tracks={tracks} context={context} />
        </section>
      )}
      <AlbumSection title="Albums" albums={albums.filter((album) => !short(album))} showArtist={false} />
      <AlbumSection title="Singles & EPs" albums={albums.filter(short)} showArtist={false} />
      <AlbumSection title="Appears on" albums={appearsOn} showArtist />
    </>
  )
}

/** A titled grid of albums, left out when there are none. */
function AlbumSection({ title, albums, showArtist }: { title: string; albums: readonly Album[]; showArtist: boolean }) {
  if (albums.length === 0) return null
  return (
    <section className="pb-6">
      <h2 className="px-6 pb-2 text-2xl font-bold">{title}</h2>
      <MediaGrid>
        {albums.map((album) => (
          <AlbumCard key={album.id} album={album} showArtist={showArtist} />
        ))}
      </MediaGrid>
    </section>
  )
}
