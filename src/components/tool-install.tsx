import { LoaderCircleIcon } from 'lucide-react'
import type { Tool, ToolInstall } from '@/bindings'
import { formatBytes } from '@/lib/format'

/** Names of the download tools as the app shows them. */
export const toolNames = { ytDlp: 'yt-dlp', ffmpeg: 'ffmpeg', deno: 'Deno' } as const satisfies Record<Tool, string>

/** How much of a tool has arrived, with the total when the server says it. */
export function installAmount(install: ToolInstall) {
  const done = formatBytes(install.doneBytes)
  return install.totalBytes == null ? done : `${done} of ${formatBytes(install.totalBytes)}`
}

/** Bar of a tool being fetched; it stays empty when the server does not say the size. */
export function InstallBar({ install }: { install: ToolInstall }) {
  const percent = install.totalBytes == null ? 0 : (install.doneBytes / install.totalBytes) * 100
  return (
    <div className="h-1 overflow-hidden rounded-full bg-foreground/20">
      <div className="h-full bg-primary transition-[width]" style={{ width: `${percent}%` }} />
    </div>
  )
}

/** Line and bar of a tool being fetched. */
export function ToolInstallProgress({ install }: { install: ToolInstall }) {
  return (
    <div className="flex flex-col gap-2">
      <p className="flex items-center gap-2 text-sm text-muted-foreground">
        <LoaderCircleIcon className="size-4 shrink-0 animate-spin" />
        Fetching {toolNames[install.tool]}, {installAmount(install)}
      </p>
      <InstallBar install={install} />
    </div>
  )
}
