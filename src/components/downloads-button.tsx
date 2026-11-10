import { useMutation, useQuery } from '@tanstack/react-query'
import { DownloadIcon, XIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import type { DownloadStage, DownloadView } from '@/bindings'
import { SourceCover } from '@/components/cover'
import { Hint } from '@/components/hint'
import { Button } from '@/components/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { formatBytes } from '@/lib/format'
import { errorMessage } from '@/lib/ipc'
import { mutations, queries } from '@/lib/query'
import { cn } from '@/lib/utils'

/** A stage a download can still leave on its own. */
export type ActiveStage = Exclude<DownloadStage, { kind: 'done' | 'failed' }>

/** Whether the download is still running. */
export function isActive(stage: DownloadStage): stage is ActiveStage {
  return stage.kind !== 'done' && stage.kind !== 'failed'
}

/** What a running download is doing, in words: how far the transfer is and how big the file is, once the site tells it. */
function stageLabel(stage: ActiveStage) {
  switch (stage.kind) {
    case 'queued':
      return 'Waiting to start'
    case 'downloading':
      return stage.totalBytes === null ? `${stage.percent}%` : `${stage.percent}% / ${formatBytes(stage.totalBytes)}`
    case 'converting':
      return 'Converting'
    case 'tagging':
      return 'Writing tags'
    default:
      return stage satisfies never
  }
}

/** Rough completion of a stage in percent; converting and tagging come after the transfer. */
function stagePercent(stage: DownloadStage) {
  switch (stage.kind) {
    case 'queued':
      return 0
    case 'downloading':
      return stage.percent * 0.9
    case 'converting':
      return 92
    case 'tagging':
    case 'done':
    case 'failed':
      return 100
    default:
      return stage satisfies never
  }
}

/** Toolbar button listing the downloads, kept across launches until dismissed; a ring around it fills while any of them runs. */
export function DownloadsButton(props: Omit<ComponentProps<typeof Button>, 'children'>) {
  const { data: downloads = [] } = useQuery(queries.downloads())
  const active = downloads.filter((d) => isActive(d.stage))
  const percent = active.length === 0 ? null : active.reduce((sum, d) => sum + stagePercent(d.stage), 0) / active.length

  return (
    <Popover>
      <Hint label="Downloads">
        <PopoverTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Downloads"
            className="relative size-9 bg-surface hover:bg-surface-hover"
            {...props}
          >
            <DownloadIcon className={cn('size-5', percent !== null && 'text-primary')} />
            {percent !== null && <ProgressRing percent={percent} />}
          </Button>
        </PopoverTrigger>
      </Hint>
      <PopoverContent className="flex w-96 flex-col gap-1">
        <h2 className="px-2 pt-1 pb-2 text-sm font-semibold">Downloads</h2>
        {downloads.length === 0 ? (
          <p className="px-2 pb-2 text-sm text-muted-foreground">
            Nothing downloaded yet. Open a playlist or a folder and press Download to get a song from YouTube or
            Spotify.
          </p>
        ) : (
          <ul className="flex max-h-96 flex-col gap-1 overflow-y-auto">
            {downloads.toReversed().map((download) => (
              <DownloadRow key={download.id} download={download} />
            ))}
          </ul>
        )}
      </PopoverContent>
    </Popover>
  )
}

function DownloadRow({ download }: { download: DownloadView }) {
  const dismiss = useMutation(mutations.dismissDownload())
  const { stage } = download
  return (
    <li className="group flex items-center gap-3 rounded-md p-2 hover:bg-surface-hover">
      <SourceCover source={download.cover} className="size-10 rounded-sm" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <span className="truncate text-sm">{download.title}</span>
        <Hint label={stage.kind === 'failed' ? errorMessage(stage.error) : download.folder}>
          <span className={cn('truncate text-xs text-muted-foreground', stage.kind === 'failed' && 'text-destructive')}>
            {download.artist}
          </span>
        </Hint>
      </div>
      {isActive(stage) ? (
        <DownloadProgress download={download} stage={stage} />
      ) : (
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={`Remove ${download.title} from the list`}
          className="opacity-0 group-hover:opacity-100"
          onClick={() => dismiss.mutate(download.id)}
        >
          <XIcon />
        </Button>
      )}
    </li>
  )
}

/** A running download's ring, filling as it goes and naming its stage on hover, and the cross that cancels it. */
export function DownloadProgress({ download, stage }: { download: DownloadView; stage: ActiveStage }) {
  const cancel = useMutation(mutations.cancelDownload())
  return (
    <>
      <Hint label={stageLabel(stage)}>
        <div className="relative size-6 shrink-0">
          <ProgressRing percent={stagePercent(stage)} thickness={3} />
        </div>
      </Hint>
      <Button
        variant="ghost"
        size="icon-sm"
        aria-label={`Cancel downloading ${download.title}`}
        onClick={() => cancel.mutate(download.id)}
      >
        <XIcon />
      </Button>
    </>
  )
}

/** Circle filling its positioned parent, filled clockwise to `percent`; `thickness` is in 36ths of its size. */
function ProgressRing({ percent, thickness = 2 }: { percent: number; thickness?: number }) {
  const radius = 18 - thickness / 2
  const circumference = 2 * Math.PI * radius
  return (
    <svg viewBox="0 0 36 36" className="pointer-events-none absolute inset-0 size-full -rotate-90">
      <circle cx="18" cy="18" r={radius} fill="none" strokeWidth={thickness} className="stroke-foreground/15" />
      <circle
        cx="18"
        cy="18"
        r={radius}
        fill="none"
        strokeWidth={thickness}
        strokeLinecap="round"
        strokeDasharray={circumference}
        strokeDashoffset={circumference * (1 - percent / 100)}
        className="stroke-primary transition-[stroke-dashoffset]"
      />
    </svg>
  )
}
