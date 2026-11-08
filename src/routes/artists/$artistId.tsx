import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Cover } from '@/components/cover'
import { AlbumCard } from '@/components/library-cards'
import { MediaGrid } from '@/components/media-card'
import { MetaDot, PageHeader } from '@/components/page-header'
import { PlayButton } from '@/components/play-button'
import { TrackTable } from '@/components/track-table'
import { plural } from '@/lib/format'
import { idParam } from '@/lib/params'
import { queries } from '@/lib/query'
import { tint } from '@/lib/tint'

export const Route = createFileRoute('/artists/$artistId')({
  params: idParam('artistId'),
  loader: ({ context, params }) => context.queryClient.ensureQueryData(queries.artist(params.artistId)),
  component: ArtistPage,
})

function ArtistPage() {
  const { artistId } = Route.useParams()
  const {
    data: { artist, albums, tracks },
  } = useSuspenseQuery(queries.artist(artistId))
  const context = { kind: 'artist', id: artist.id } as const

  return (
    <>
      <PageHeader
        eyebrow="Artist"
        title={artist.name}
        tint={tint(artist.id)}
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
      </PageHeader>
      {albums.length > 0 && (
        <section className="pb-6">
          <h2 className="px-6 pb-2 text-2xl font-bold">Discography</h2>
          <MediaGrid>
            {albums.map((album) => (
              <AlbumCard key={album.id} album={album} showArtist={false} />
            ))}
          </MediaGrid>
        </section>
      )}
      <h2 className="px-6 pb-2 text-2xl font-bold">Songs</h2>
      <TrackTable tracks={tracks} context={context} />
    </>
  )
}
