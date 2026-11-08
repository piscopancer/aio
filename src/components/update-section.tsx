import { useQuery } from '@tanstack/react-query'
import { Button } from '@/components/ui/button'
import { queries } from '@/lib/query'
import { type UpdateStatus, checkForUpdates, installUpdate, useUpdates } from '@/lib/updates'

/** One line on where the app stands with updates. */
function describe(status: UpdateStatus) {
  switch (status.kind) {
    case 'idle':
      return 'Not checked yet.'
    case 'checking':
      return 'Checking for updates…'
    case 'latest':
      return 'You are on the latest version.'
    case 'downloading':
      return `Downloading version ${status.version}, ${Math.floor(status.percent)}%.`
    case 'ready':
      return `Version ${status.version} is ready to install.`
    case 'failed':
      return `Update failed: ${status.message}`
    case 'unsupported':
      return 'Updates are only available in the installed app.'
    default:
      return status satisfies never
  }
}

/** The installed version, its update status and the button that moves it along. */
export function UpdateSection() {
  const status = useUpdates()
  const { data: version } = useQuery(queries.appVersion())
  const busy = status.kind === 'checking' || status.kind === 'downloading'
  return (
    <div className="flex items-center justify-between gap-4">
      <div className="grid gap-0.5">
        <span className="text-sm font-semibold">
          Version
          {version !== undefined && <span className="ml-2 font-normal text-muted-foreground">{version}</span>}
        </span>
        <p className="text-xs text-muted-foreground">{describe(status)}</p>
      </div>
      {status.kind === 'ready' ? (
        <Button size="sm" onClick={() => void installUpdate()}>
          Restart and update
        </Button>
      ) : (
        <Button
          size="sm"
          variant="outline"
          disabled={busy || status.kind === 'unsupported'}
          onClick={() => void checkForUpdates()}
        >
          Check now
        </Button>
      )}
    </div>
  )
}
