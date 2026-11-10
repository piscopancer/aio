import { useMutation, useQuery } from '@tanstack/react-query'
import { open } from '@tauri-apps/plugin-dialog'
import { FolderIcon, LoaderCircleIcon, RefreshCwIcon } from 'lucide-react'
import type { ToolVersions, ToolsView } from '@/bindings'
import { Field } from '@/components/settings-field'
import { InstallBar, installAmount, toolNames } from '@/components/tool-install'
import { Button } from '@/components/ui/button'
import { mutations, queries } from '@/lib/query'

/** Cards of the tools, in the order downloads use them. */
const tools = [
  { key: 'ytDlp', role: 'reads YouTube and YouTube Music' },
  { key: 'ffmpeg', role: 'converts and repairs audio' },
  { key: 'deno', role: 'runs the scripts YouTube requires' },
] as const satisfies readonly { key: keyof ToolVersions; role: string }[]

/** Downloads tab of the settings: the default folder and the tools downloads run on. */
export function DownloadSettings() {
  const { data: settings } = useQuery(queries.downloadSettings())
  const { data: toolsView } = useQuery(queries.downloadTools())
  const setFolder = useMutation(mutations.setDownloadFolder())
  const update = useMutation(mutations.updateDownloadTools())
  const installing = toolsView?.installing ?? null
  const busy = update.isPending || toolsView?.updating === true || installing !== null
  const complete = toolsView !== undefined && tools.every((tool) => toolsView.versions[tool.key] != null)

  return (
    <div className="grid gap-6">
      <Field
        title="Download folder"
        hint="New songs are saved here unless a download picks another folder. Folders outside your library are added to it."
      >
        <div className="flex items-center gap-3 rounded-md bg-secondary px-3 py-2 text-sm">
          <FolderIcon className="size-4 shrink-0 text-muted-foreground" />
          <span className="min-w-0 flex-1 truncate select-text">{settings?.folder ?? '…'}</span>
          <Button variant="ghost" size="sm" onClick={() => setFolder.mutate(null)}>
            Default
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={async () => {
              const picked = await open({
                directory: true,
                title: 'Save downloads to',
                ...(settings && { defaultPath: settings.folder }),
              })
              if (picked !== null) setFolder.mutate(picked)
            }}
          >
            Change
          </Button>
        </div>
      </Field>
      <Field
        title="Tools"
        hint="Fetched the first time you download and checked for updates once a day, since YouTube changes often."
      >
        <ul className="grid gap-2">
          {tools.map((tool) => (
            <ToolCard key={tool.key} tool={tool} view={toolsView} />
          ))}
        </ul>
        <div className="flex items-center gap-3">
          <Button variant="outline" size="sm" disabled={busy} onClick={() => update.mutate()}>
            <RefreshCwIcon className={busy ? 'animate-spin' : undefined} />
            {installing !== null
              ? 'Installing'
              : busy
                ? 'Updating'
                : complete
                  ? 'Update tools'
                  : 'Install missing tools'}
          </Button>
          {toolsView?.checkedAt != null && (
            <span className="text-xs text-muted-foreground">
              Last checked {new Date(toolsView.checkedAt * 1000).toLocaleString()}
            </span>
          )}
        </div>
      </Field>
    </div>
  )
}

/** One tool with its role and version, or the progress of fetching or updating it. */
function ToolCard({ tool, view }: { tool: (typeof tools)[number]; view: ToolsView | undefined }) {
  const install = view?.installing?.tool === tool.key ? view.installing : null
  // only yt-dlp updates itself; the other tools are fetched once
  const updating = tool.key === 'ytDlp' && view?.updating === true && install === null
  const version = view?.versions[tool.key] ?? null
  return (
    <li className="flex flex-col gap-3 rounded-lg border bg-card px-4 py-3 text-sm">
      <div className="flex items-center gap-4">
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="font-semibold">{toolNames[tool.key]}</span>
          <span className="text-muted-foreground">{tool.role}</span>
        </div>
        <span className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground tabular-nums">
          {(install !== null || updating) && <LoaderCircleIcon className="size-3.5 shrink-0 animate-spin" />}
          <span className="truncate select-text">
            {install !== null
              ? installAmount(install)
              : updating
                ? 'Checking for updates'
                : (version ?? 'Not installed')}
          </span>
        </span>
      </div>
      {install !== null && <InstallBar install={install} />}
    </li>
  )
}
