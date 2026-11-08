import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Music2Icon } from 'lucide-react'
import type { PlaybackContext } from '@/bindings'
import { EmptyLibrary } from '@/components/empty-state'
import { MetaDot, PageHeader } from '@/components/page-header'
import { PlayButton } from '@/components/play-button'
import { TrackTable } from '@/components/track-table'
import { formatLength, plural } from '@/lib/format'
import { queries } from '@/lib/query'
import { libraryTint } from '@/lib/tint'

export const Route = createFileRoute('/')({
  loader: ({ context }) => context.queryClient.ensureQueryData(queries.tracks()),
  component: Songs,
})

const context = { kind: 'songs' } as const satisfies PlaybackContext

function Songs() {
  const { data: tracks } = useSuspenseQuery(queries.tracks())
  const totalMs = tracks.reduce((sum, track) => sum + track.durationMs, 0)

  return (
    <>
      <PageHeader
        eyebrow="Library"
        title="Songs"
        tint={libraryTint}
        cover={
          <div className="flex items-center justify-center rounded-md bg-linear-135 from-primary to-primary-foreground">
            <Music2Icon className="size-1/3 text-foreground" />
          </div>
        }
        meta={
          <>
            <span className="font-semibold text-foreground">{plural(tracks.length, 'song')}</span>
            {tracks.length > 0 && <MetaDot />}
            {tracks.length > 0 && <span>{formatLength(totalMs)}</span>}
          </>
        }
      >
        <PlayButton context={context} disabled={tracks.length === 0} />
      </PageHeader>
      {tracks.length === 0 ? <EmptyLibrary /> : <TrackTable tracks={tracks} context={context} />}
    </>
  )
}
