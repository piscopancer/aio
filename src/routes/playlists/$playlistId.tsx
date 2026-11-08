import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { DownloadIcon, EllipsisIcon, PencilIcon, Trash2Icon } from 'lucide-react'
import { useState } from 'react'
import { Cover } from '@/components/cover'
import { DownloadDialog } from '@/components/download-dialog'
import { EmptyState } from '@/components/empty-state'
import { MetaDot, PageHeader } from '@/components/page-header'
import { PinButton } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { type PlaylistDialog, PlaylistDialogs } from '@/components/playlist-dialogs'
import { TrackTable } from '@/components/track-table'
import { Button } from '@/components/ui/button'
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from '@/components/ui/dropdown-menu'
import { formatLength, plural } from '@/lib/format'
import { idParam } from '@/lib/params'
import { queries } from '@/lib/query'

export const Route = createFileRoute('/playlists/$playlistId')({
  params: idParam('playlistId'),
  loader: ({ context, params }) => context.queryClient.ensureQueryData(queries.playlist(params.playlistId)),
  component: PlaylistPage,
})

function PlaylistPage() {
  const { playlistId } = Route.useParams()
  const {
    data: { playlist, tracks },
  } = useSuspenseQuery(queries.playlist(playlistId))
  const [dialog, setDialog] = useState<PlaylistDialog>(null)
  const context = { kind: 'playlist', id: playlist.id } as const

  return (
    <>
      <PageHeader
        eyebrow="Playlist"
        title={playlist.name}
        art={playlist.cover}
        cover={<Cover src={playlist.cover} kind="playlist" />}
        onTitleClick={() => setDialog('edit')}
        meta={
          <>
            <span>{plural(playlist.trackCount, 'song')}</span>
            {playlist.trackCount > 0 && <MetaDot />}
            {playlist.trackCount > 0 && <span>{formatLength(playlist.durationMs)}</span>}
          </>
        }
      >
        <PlayButton context={context} disabled={tracks.length === 0} />
        <PinButton target={{ kind: 'playlist', id: playlist.id }} />
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" aria-label={`More options for ${playlist.name}`} className="size-10">
              <EllipsisIcon className="size-7" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onSelect={() => setDialog('edit')}>
              <PencilIcon />
              Edit details
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => setDialog('delete')}>
              <Trash2Icon />
              Delete
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <DownloadDialog origin={{ playlistId: playlist.id }}>
          <Button variant="outline" className="ml-auto">
            <DownloadIcon />
            Download
          </Button>
        </DownloadDialog>
      </PageHeader>
      {tracks.length === 0 ? (
        <EmptyState title="This playlist is empty">
          Drag songs onto the playlist in the sidebar, right-click a song and choose Add to playlist, or press Download
          to get one from YouTube or Spotify.
        </EmptyState>
      ) : (
        <TrackTable tracks={tracks} context={context} />
      )}
      <PlaylistDialogs playlist={playlist} open={dialog} onClose={() => setDialog(null)} />
    </>
  )
}
