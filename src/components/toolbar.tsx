import { useQuery } from '@tanstack/react-query'
import { useCanGoBack, useRouter } from '@tanstack/react-router'
import { ChevronLeftIcon, ChevronRightIcon, type LucideIcon, SettingsIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import { DownloadsButton } from '@/components/downloads-button'
import { Hint } from '@/components/hint'
import { Search } from '@/components/search'
import { SettingsDialog } from '@/components/settings-dialog'
import { Button } from '@/components/ui/button'
import { BACK_KEYS, FORWARD_KEYS } from '@/lib/layout'
import { queries } from '@/lib/query'

/** App bar that also serves as the window's title bar: the brand and history arrows, the search, then the settings; empty stretches drag the window. */
export function Toolbar() {
  const { history } = useRouter()
  const canGoBack = useCanGoBack()
  return (
    <header
      data-tauri-drag-region
      onMouseDownCapture={(event) => {
        // Tauri cancels the press on a drag stretch to move the window, which would keep the search focused
        const { target } = event
        const focused = document.activeElement
        if (
          target instanceof HTMLElement &&
          target.hasAttribute('data-tauri-drag-region') &&
          focused instanceof HTMLElement
        )
          focused.blur()
      }}
      className="flex h-toolbar shrink-0 items-center gap-2 title-bar pl-5"
    >
      <Brand />
      <div className="ml-4 flex gap-2">
        <ToolButton
          icon={ChevronLeftIcon}
          label="Back"
          keys={BACK_KEYS}
          disabled={!canGoBack}
          onClick={() => history.back()}
        />
        <ToolButton icon={ChevronRightIcon} label="Forward" keys={FORWARD_KEYS} onClick={() => history.forward()} />
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

/** Round icon button of the toolbar with its label as a hint; extra props reach the button, so it can serve as a trigger. */
function ToolButton({
  icon: Icon,
  label,
  keys,
  ...props
}: Omit<ComponentProps<typeof Button>, 'children'> & {
  icon: LucideIcon
  label: string
  keys?: ComponentProps<typeof Hint>['keys']
}) {
  return (
    <Hint label={label} {...(keys && { keys })}>
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
