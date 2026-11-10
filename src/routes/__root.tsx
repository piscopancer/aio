import type { QueryClient } from '@tanstack/react-query'
import { Link, createRootRouteWithContext } from '@tanstack/react-router'
import { useState } from 'react'
import { LibraryTabs } from '@/components/library-tabs'
import { PlayerBar } from '@/components/player-bar'
import { ErrorDialog } from '@/components/error-dialog'
import { LyricsPanel, LyricsStage } from '@/components/lyrics-panel'
import { MainView } from '@/components/main-view'
import { QueuePanel } from '@/components/queue-panel'
import { Sidebar } from '@/components/sidebar'
import { Button } from '@/components/ui/button'
import { Toolbar } from '@/components/toolbar'
import { TrackEditDialog } from '@/components/track-edit-dialog'
import { ScrollArea } from '@/components/ui/scroll-area'
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
  const panel = useLayout((state) => state.panel)
  const lyricsView = useLayout((state) => (state.panel === 'lyrics' ? state.lyricsView : null))
  useAppShortcuts()

  return (
    <TooltipProvider>
      <div className="flex h-full flex-col">
        <Toolbar />
        <div className="grid min-h-0 flex-1 grid-rows-[minmax(0,1fr)_auto] gap-2 px-2">
          <div className="flex min-h-0 gap-2">
            <Sidebar />
            <div className="relative flex min-w-0 flex-1 gap-2">
              <div className="flex min-w-0 flex-1 flex-col overflow-hidden rounded-lg bg-surface">
                <LibraryTabs />
                <main className="min-h-0 flex-1">
                  <ScrollArea viewportRef={setScrollElement} className="h-full">
                    <ScrollContainerContext value={scrollElement}>
                      <MainView />
                    </ScrollContainerContext>
                  </ScrollArea>
                </main>
              </div>
              {panel === 'queue' && <QueuePanel />}
              {panel === 'lyrics' && <LyricsPanel />}
              {lyricsView === 'expanded' && <LyricsStage view="expanded" />}
            </div>
          </div>
          <PlayerBar />
        </div>
      </div>
      {lyricsView === 'fullscreen' && <LyricsStage view="fullscreen" />}
      <TrackEditDialog />
      <ErrorDialog />
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
