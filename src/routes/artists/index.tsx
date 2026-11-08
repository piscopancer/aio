import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { EmptyLibrary } from '@/components/empty-state'
import { ArtistCard } from '@/components/library-cards'
import { MediaGrid } from '@/components/media-card'
import { queries } from '@/lib/query'

export const Route = createFileRoute('/artists/')({
  loader: ({ context }) => context.queryClient.ensureQueryData(queries.artists()),
  component: Artists,
})

function Artists() {
  const { data: artists } = useSuspenseQuery(queries.artists())
  return (
    <div className="py-6">
      <h1 className="px-6 pb-4 text-3xl font-bold">Artists</h1>
      {artists.length === 0 ? (
        <EmptyLibrary />
      ) : (
        <MediaGrid>
          {artists.map((artist) => (
            <ArtistCard key={artist.id} artist={artist} />
          ))}
        </MediaGrid>
      )}
    </div>
  )
}
