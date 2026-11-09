import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { EmptyLibrary } from '@/components/empty-state'
import { AlbumCard } from '@/components/library-cards'
import { MediaGrid } from '@/components/media-card'
import { PIN_IDLE, pinDropMarks, splitPinned, usePinReorder } from '@/lib/pins'
import { queries } from '@/lib/query'

export const Route = createFileRoute('/albums/')({
  loader: ({ context }) => context.queryClient.ensureQueryData(queries.albums()),
  component: Albums,
})

function Albums() {
  const { data: albums } = useSuspenseQuery(queries.albums())
  const { pinned, rest } = splitPinned(albums)
  const dragProps = usePinReorder(
    'album',
    pinned.map((item) => item.id),
    'grid',
  )
  return (
    <div className="py-6">
      <h1 className="px-6 pb-4 text-3xl font-bold">Albums</h1>
      {albums.length === 0 ? (
        <EmptyLibrary />
      ) : (
        <MediaGrid>
          {pinned.map((album) => (
            <AlbumCard key={album.id} album={album} {...dragProps(album.id)} className={pinDropMarks.grid} />
          ))}
          {rest.map((album) => (
            <AlbumCard key={album.id} album={album} className={PIN_IDLE} />
          ))}
        </MediaGrid>
      )}
    </div>
  )
}
