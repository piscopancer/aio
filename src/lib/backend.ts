import { toast } from 'sonner'
import { create } from 'zustand'
import { type DownloadView, type ScanProgress, events } from '@/bindings'
import { errorMessage } from '@/lib/ipc'
import { syncPlayer } from '@/lib/player'
import { invalidateLibrary, queries, queryClient } from '@/lib/query'

/** Progress of the running library scan, `null` while no scan runs. */
export const useScan = create<ScanProgress | null>()(() => null)

/** Connects the frontend to every backend event once, at startup. */
export async function connectBackend() {
  await Promise.all([
    syncPlayer(),
    events.libraryChanged.listen(() => void invalidateLibrary()),
    events.playbackFailed.listen(({ payload }) => toast.error(errorMessage(payload))),
    events.scanProgress.listen(({ payload }) => useScan.setState(payload.done < payload.total ? payload : null, true)),
    events.downloadsChanged.listen(({ payload }) => {
      const { queryKey } = queries.downloads()
      announceFinished(queryClient.getQueryData(queryKey) ?? [], payload)
      queryClient.setQueryData(queryKey, payload)
    }),
    events.toolsChanged.listen(({ payload }) => queryClient.setQueryData(queries.downloadTools().queryKey, payload)),
  ])
}

/** Toasts each download that has just finished or failed. */
function announceFinished(before: DownloadView[], after: DownloadView[]) {
  for (const download of after) {
    const previous = before.find((d) => d.id === download.id)?.stage.kind
    if (previous === download.stage.kind) continue
    if (download.stage.kind === 'done') toast.success(`Downloaded ${download.title}`)
    else if (download.stage.kind === 'failed')
      toast.error(`${download.title} failed: ${errorMessage(download.stage.error)}`)
  }
}
