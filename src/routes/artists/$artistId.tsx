import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import type { Album } from '@/bindings'
import { Cover } from '@/components/cover'
import { AlbumCard } from '@/components/library-cards'
import { MediaGrid } from '@/components/media-card'
import { MetaDot, PageHeader } from '@/components/page-header'
import { PinButton } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { TrackTable } from '@/components/track-table'
import { plural } from '@/lib/format'
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

  return (
    <>
      <PageHeader
        eyebrow="Artist"
        title={artist.name}
        art={artist.cover}
        cover={<Cover src={artist.cover} kind="artist" />}
        meta={
          <>
            <span>{plural(artist.albumCount, 'album')}</span>
            <MetaDot />
            <span>{plural(artist.trackCount, 'song')}</span>
          </>
        }
      >
        <PlayButton context={context} />
        <PinButton target={{ kind: 'artist', id: artist.id }} />
      </PageHeader>
      <section className="pb-6">
        <TrackTable tracks={tracks} context={context} />
      </section>
      <AlbumSection title="Discography" albums={albums} showArtist={false} />
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
