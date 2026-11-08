import { useQuery } from '@tanstack/react-query'
import { useCanGoBack, useRouter } from '@tanstack/react-router'
import { ChevronLeftIcon, ChevronRightIcon, type LucideIcon, SearchIcon, SettingsIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import { DownloadsButton } from '@/components/downloads-button'
import { Hint } from '@/components/hint'
import { SettingsDialog } from '@/components/settings-dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { queries } from '@/lib/query'

/** App bar that also serves as the window's title bar: the brand and history arrows, the search, then the settings; empty stretches drag the window. */
export function Toolbar() {
  const { history } = useRouter()
  const canGoBack = useCanGoBack()
  return (
    <header data-tauri-drag-region className="flex h-toolbar shrink-0 items-center gap-2 title-bar pl-5">
      <Brand />
      <div className="ml-4 flex gap-2">
        <ToolButton icon={ChevronLeftIcon} label="Back (Alt+←)" disabled={!canGoBack} onClick={() => history.back()} />
        <ToolButton icon={ChevronRightIcon} label="Forward (Alt+→)" onClick={() => history.forward()} />
      </div>
      <div data-tauri-drag-region className="flex flex-1 justify-center self-stretch px-4">
        <Search />
      </div>
      <DownloadsButton />
      <SettingsDialog>
        <ToolButton icon={SettingsIcon} label="Settings" />
      </SettingsDialog>
    </header>
  )
}

/** Logo, name and the dev scenario badge; the name lets clicks through so it drags the window too. */
function Brand() {
  const { data: scenario } = useQuery(queries.scenario())
  return (
    <div className="flex items-center gap-2">
      <img src="/app-icon.svg" alt="" className="pointer-events-none size-7" />
      <span className="pointer-events-none text-xl font-black tracking-tight">Aio</span>
      {scenario != null && (
        <Hint label={`Started with --scenario ${scenario}, changes are not saved`}>
          <span className="rounded-full border border-primary/60 px-2 py-0.5 text-xs text-primary">{scenario}</span>
        </Hint>
      )}
    </div>
  )
}

/** Library search; a placeholder until searching exists. */
function Search() {
  return (
    <label className="relative flex w-full max-w-md items-center self-center">
      <SearchIcon className="pointer-events-none absolute left-3 size-5 text-muted-foreground" />
      <Input disabled placeholder="Search is coming soon" className="h-10 rounded-full bg-surface pl-11" />
    </label>
  )
}

/** Round icon button of the toolbar with its label as a hint; extra props reach the button, so it can serve as a trigger. */
function ToolButton({
  icon: Icon,
  label,
  ...props
}: Omit<ComponentProps<typeof Button>, 'children'> & { icon: LucideIcon; label: string }) {
  return (
    <Hint label={label}>
      <Button
        variant="ghost"
        size="icon"
        aria-label={label}
        className="size-9 bg-surface hover:bg-surface-hover"
        {...props}
      >
        <Icon className="size-5" />
      </Button>
    </Hint>
  )
}
