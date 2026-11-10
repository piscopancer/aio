import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { ArtistLinks } from '@/components/artist-menu'
import { Cover } from '@/components/cover'
import { MetaStat, MetaStats, PageHeader } from '@/components/page-header'
import { PinButton } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { TrackTable } from '@/components/track-table'
import { releaseLabel } from '@/lib/format'
import { idParam } from '@/lib/params'
import { queries } from '@/lib/query'

export const Route = createFileRoute('/albums/$albumId')({
  params: idParam('albumId'),
  loader: ({ context, params }) => context.queryClient.ensureQueryData(queries.album(params.albumId)),
  component: AlbumPage,
})

function AlbumPage() {
  const { albumId } = Route.useParams()
  const {
    data: { album, tracks },
  } = useSuspenseQuery(queries.album(albumId))
  const context = { kind: 'album', id: album.id } as const

  return (
    <>
      <PageHeader
        eyebrow={releaseLabel(album)}
        title={album.title}
        art={album.cover}
        cover={<Cover src={album.cover} kind="album" />}
        meta={
          <>
            {album.artists.length > 0 && (
              <ArtistLinks artists={album.artists} className="min-w-0 font-semibold text-foreground" />
            )}
            <MetaStats>
              {album.year !== null && <MetaStat kind="year" value={album.year} />}
              <MetaStat kind="songs" value={album.trackCount} />
              <MetaStat kind="length" value={album.durationMs} />
            </MetaStats>
          </>
        }
      >
        <PlayButton context={context} />
        <PinButton target={{ kind: 'album', id: album.id }} />
      </PageHeader>
      <TrackTable tracks={tracks} context={context} />
    </>
  )
}
