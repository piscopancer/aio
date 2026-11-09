import { useSuspenseQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { Cover } from '@/components/cover'
import { MetaDot, PageHeader } from '@/components/page-header'
import { PinButton } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { TrackTable } from '@/components/track-table'
import { formatLength, plural } from '@/lib/format'
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
        eyebrow="Album"
        title={album.title}
        art={album.cover}
        cover={<Cover src={album.cover} kind="album" />}
        meta={
          <>
            {album.artistId !== null && (
              <>
                <Link
                  to="/artists/$artistId"
                  params={{ artistId: album.artistId }}
                  className="font-semibold text-foreground hover:underline"
                >
                  {album.artist}
                </Link>
                <MetaDot />
              </>
            )}
            {album.year !== null && (
              <>
                <span>{album.year}</span>
                <MetaDot />
              </>
            )}
            <span>
              {plural(album.trackCount, 'song')}, {formatLength(album.durationMs)}
            </span>
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
