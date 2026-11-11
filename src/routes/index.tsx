import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { CirclePlusIcon, HistoryIcon, type LucideIcon } from 'lucide-react'
import type { HomeEntry, PlaybackContext } from '@/bindings'
import { EmptyLibrary } from '@/components/empty-state'
import { AlbumCard, ArtistCard, PlaylistCard, TrackCard } from '@/components/library-cards'
import { CardRow } from '@/components/media-card'
import { queries } from '@/lib/query'

export const Route = createFileRoute('/')({
  loader: ({ context }) =>
    Promise.all([
      context.queryClient.ensureQueryData(queries.tracks()),
      context.queryClient.ensureQueryData(queries.home()),
    ]),
  component: Home,
})

/** What the song cards of each section play from, kept whole so the memoized cards keep it across renders. */
const recentContext = { kind: 'recent' } as const satisfies PlaybackContext
const addedContext = { kind: 'added' } as const satisfies PlaybackContext

/** The page the app opens on, gathering what is most likely wanted next. */
function Home() {
  const { data: hasTracks } = useSuspenseQuery({ ...queries.tracks(), select: (tracks) => tracks.length > 0 })
  const { data: home } = useSuspenseQuery({
    ...queries.home(),
    select: ({ recent, added }) => ({
      recent,
      added: added.map((track): HomeEntry => ({ kind: 'track', track })),
    }),
  })

  if (!hasTracks) {
    return (
      <div className="py-6">
        <EmptyLibrary />
      </div>
    )
  }
  return (
    <div className="flex flex-col gap-8 py-6">
      <HomeSection
        title="Recently played"
        icon={HistoryIcon}
        context={recentContext}
        entries={home.recent}
        empty="Songs you play for half a minute appear here, along with their albums, their artists and the playlists you play them from."
      />
      <HomeSection
        title="Recently added"
        icon={CirclePlusIcon}
        context={addedContext}
        entries={home.added}
        empty="Songs you download or add to your folders appear here."
      />
    </div>
  )
}

/** One section of the home page as one row of cards; its songs play from `context`, in the order of the section. */
function HomeSection({
  title,
  icon: Icon,
  context,
  entries,
  empty,
}: {
  title: string
  icon: LucideIcon
  context: PlaybackContext
  entries: HomeEntry[]
  empty: string
}) {
  const heading = (
    <h2 className="flex items-center gap-3 text-2xl font-bold">
      <Icon strokeWidth={1.5} className="size-6 shrink-0 text-muted-foreground" />
      {title}
    </h2>
  )
  return (
    <section>
      {entries.length === 0 ? (
        <>
          <div className="px-6 pb-4">{heading}</div>
          <p className="px-6 text-muted-foreground">{empty}</p>
        </>
      ) : (
        <CardRow header={heading}>
          {entries.map((entry) => (
            <HomeCard key={entryKey(entry)} entry={entry} context={context} />
          ))}
        </CardRow>
      )}
    </section>
  )
}

function HomeCard({ entry, context }: { entry: HomeEntry; context: PlaybackContext }) {
  switch (entry.kind) {
    case 'track':
      return <TrackCard track={entry.track} context={context} />
    case 'album':
      return <AlbumCard album={entry.album} />
    case 'artist':
      return <ArtistCard artist={entry.artist} />
    case 'playlist':
      return <PlaylistCard playlist={entry.playlist} />
    default:
      return entry satisfies never
  }
}

/** Kind and id, unique across the kinds a section mixes. */
function entryKey(entry: HomeEntry) {
  switch (entry.kind) {
    case 'track':
      return `track:${entry.track.id}`
    case 'album':
      return `album:${entry.album.id}`
    case 'artist':
      return `artist:${entry.artist.id}`
    case 'playlist':
      return `playlist:${entry.playlist.id}`
    default:
      return entry satisfies never
  }
}
