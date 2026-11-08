import type { QueryClient } from '@tanstack/react-query'
import { Link, Outlet, createRootRouteWithContext, useRouterState } from '@tanstack/react-router'
import { useEffect, useState } from 'react'
import { LibraryTabs } from '@/components/library-tabs'
import { PlayerBar } from '@/components/player-bar'
import { QueuePanel } from '@/components/queue-panel'
import { Sidebar } from '@/components/sidebar'
import { Button } from '@/components/ui/button'
import { Toolbar } from '@/components/toolbar'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import { useLayout } from '@/lib/layout'
import { useAppShortcuts } from '@/lib/shortcuts'
import { ScrollContainerContext } from '@/lib/virtual'

export const Route = createRootRouteWithContext<{ queryClient: QueryClient }>()({
  component: Shell,
  notFoundComponent: NotFound,
})

/** Window layout: sidebar, scrolling main view and the optional queue on top, the player bar below. */
function Shell() {
  const [scrollElement, setScrollElement] = useState<HTMLElement | null>(null)
  const queueOpen = useLayout((state) => state.queueOpen)
  const pathname = useRouterState({ select: (state) => state.location.pathname })
  useAppShortcuts()

  useEffect(() => {
    scrollElement?.scrollTo({ top: 0 })
  }, [scrollElement, pathname])

  return (
    <TooltipProvider>
      <div className="flex h-full flex-col">
        <Toolbar />
        <div className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)_auto] gap-2 px-2">
          <div className="flex min-h-0 gap-2">
            <Sidebar />
            <div className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg bg-surface">
              <LibraryTabs />
              <main ref={setScrollElement} className="min-h-0 flex-1 overflow-y-auto">
                <ScrollContainerContext value={scrollElement}>
                  <Outlet />
                </ScrollContainerContext>
              </main>
            </div>
            {queueOpen && <QueuePanel />}
          </div>
          <PlayerBar />
        </div>
      </div>
      <Toaster />
    </TooltipProvider>
  )
}

function NotFound() {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4">
      <h1 className="text-3xl font-bold">Not found</h1>
      <p className="text-muted-foreground">It may have been removed from the library.</p>
      <Button variant="light" asChild>
        <Link to="/">Back to songs</Link>
      </Button>
    </div>
  )
}
