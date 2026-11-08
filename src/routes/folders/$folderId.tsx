import { useSuspenseQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { DownloadIcon } from 'lucide-react'
import { Cover } from '@/components/cover'
import { DownloadDialog } from '@/components/download-dialog'
import { EmptyState } from '@/components/empty-state'
import { MetaDot, PageHeader } from '@/components/page-header'
import { PlayButton } from '@/components/play-button'
import { TrackTable } from '@/components/track-table'
import { Button } from '@/components/ui/button'
import { plural } from '@/lib/format'
import { idParam } from '@/lib/params'
import { queries } from '@/lib/query'
import { tint } from '@/lib/tint'

export const Route = createFileRoute('/folders/$folderId')({
  params: idParam('folderId'),
  loader: ({ context, params }) => context.queryClient.ensureQueryData(queries.folder(params.folderId)),
  component: FolderPage,
})

/** Last part of a folder path, the name the folder goes by. */
function folderName(path: string) {
  return path.split(/[\/]/).findLast((part) => part !== '') ?? path
}

function FolderPage() {
  const { folderId } = Route.useParams()
  const {
    data: { folder, tracks },
  } = useSuspenseQuery(queries.folder(folderId))
  const context = { kind: 'folder', id: folder.id } as const

  return (
    <>
      <PageHeader
        eyebrow="Folder"
        title={folderName(folder.path)}
        tint={tint(folder.id)}
        cover={<Cover src={null} kind="folder" />}
        meta={
          <>
            <span className="truncate select-text">{folder.path}</span>
            <MetaDot />
            <span className="shrink-0">{plural(folder.trackCount, 'song')}</span>
          </>
        }
      >
        <PlayButton context={context} disabled={tracks.length === 0} />
        <DownloadDialog origin={{ folder: folder.path }}>
          <Button variant="outline" className="ml-auto">
            <DownloadIcon />
            Download
          </Button>
        </DownloadDialog>
      </PageHeader>
      {tracks.length === 0 ? (
        <EmptyState title="No songs in this folder">Press Download to get a song from YouTube or Spotify.</EmptyState>
      ) : (
        <TrackTable tracks={tracks} context={context} />
      )}
    </>
  )
}
