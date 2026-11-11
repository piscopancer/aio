import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Music2Icon } from 'lucide-react'
import { EmptyLibrary } from '@/components/empty-state'
import { MetaStat, MetaStats, PageHeader } from '@/components/page-header'
import { PlayButton } from '@/components/play-button'
import { TrackTable } from '@/components/track-table'
import { totalDuration } from '@/lib/format'
import { songsContext as context } from '@/lib/player'
import { queries } from '@/lib/query'
import { libraryTint } from '@/lib/tint'

export const Route = createFileRoute('/songs')({
  loader: ({ context }) => context.queryClient.ensureQueryData(queries.tracks()),
  component: Songs,
})

function Songs() {
  const { data: tracks } = useSuspenseQuery(queries.tracks())

  return (
    <>
      <PageHeader
        eyebrow="Library"
        title="Songs"
        tint={libraryTint}
        cover={
          <div className="flex items-center justify-center rounded-md bg-linear-135 from-[oklch(from_var(--primary)_calc(l+0.08)_calc(c*0.6)_h)] to-primary">
            <Music2Icon className="size-1/3 text-primary-foreground" />
          </div>
        }
        meta={
          <>
            <MetaStats>
              <MetaStat kind="songs" value={tracks.length} />
              {tracks.length > 0 && <MetaStat kind="length" value={totalDuration(tracks)} />}
            </MetaStats>
          </>
        }
      >
        <PlayButton context={context} disabled={tracks.length === 0} />
      </PageHeader>
      {tracks.length === 0 ? <EmptyLibrary /> : <TrackTable tracks={tracks} context={context} />}
    </>
  )
}
