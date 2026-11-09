import { XIcon } from 'lucide-react'
import { memo, useState } from 'react'
import type { QueueItem } from '@/bindings'
import { Cover } from '@/components/cover'
import { Equalizer } from '@/components/equalizer'
import { Hint } from '@/components/hint'
import { Marquee } from '@/components/marquee'
import { Button } from '@/components/ui/button'
import { togglePanel } from '@/lib/layout'
import { currentItem, player, usePlayer } from '@/lib/player'
import { credit } from '@/lib/format'
import { cn } from '@/lib/utils'
import { useListVirtualizer } from '@/lib/virtual'

const ROW_HEIGHT = 64

/** Right panel with the playing track and what plays after it, in queue order. */
export function QueuePanel() {
  const [scrollElement, setScrollElement] = useState<HTMLElement | null>(null)
  const current = usePlayer(currentItem)
  const index = usePlayer((state) => state.index)
  const queue = usePlayer((state) => state.queue)
  const upcoming = index === null ? [] : queue.slice(index + 1)
  const { listRef, items, totalSize, scrollMargin } = useListVirtualizer(scrollElement, upcoming.length, ROW_HEIGHT)

  return (
    <aside ref={setScrollElement} className="w-80 shrink-0 overflow-y-auto rounded-lg bg-surface">
      <div>
        <header className="sticky top-0 z-10 flex items-center justify-between bg-surface px-4 py-3">
          <h2 className="font-bold">Queue</h2>
          <Hint label="Close">
            <Button variant="ghost" size="icon" className="hover:bg-surface-hover" onClick={() => togglePanel('queue')}>
              <XIcon />
            </Button>
          </Hint>
        </header>
        {current === undefined ? (
          <p className="px-4 text-sm text-muted-foreground">Nothing is playing. Double-click a song to start.</p>
        ) : (
          <>
            <h3 className="px-4 pb-1 font-semibold">Now playing</h3>
            <div className="px-2">
              <QueueRow item={current} current />
            </div>
            {upcoming.length > 0 && <h3 className="px-4 pt-4 pb-1 font-semibold">Next up</h3>}
          </>
        )}
        <div ref={listRef} className="relative mx-2 mb-2" style={{ height: totalSize }}>
          {items.map((row) => {
            const item = upcoming[row.index]
            if (item === undefined) return null
            return (
              <div
                key={item.key}
                className="absolute top-0 left-0 w-full"
                style={{ transform: `translateY(${row.start - scrollMargin}px)` }}
              >
                <QueueRow item={item} />
              </div>
            )
          })}
        </div>
      </div>
    </aside>
  )
}

/** One queue entry; memoized, so scrolling the panel only moves rows instead of rendering them again. */
const QueueRow = memo(function QueueRow({ item, current = false }: { item: QueueItem; current?: boolean }) {
  const isPlaying = usePlayer((state) => current && state.isPlaying)
  const { track } = item
  return (
    <div
      onDoubleClick={() => (current ? player.toggle() : player.jump(item.key))}
      className="group flex items-center gap-3 rounded-md p-2 hover:bg-surface-hover"
      style={{ height: ROW_HEIGHT }}
    >
      <Cover src={track.cover} kind="track" className="size-12" />
      <div className="flex min-w-0 flex-1 flex-col">
        <Marquee className={cn(current && 'text-primary')}>{track.title}</Marquee>
        {track.artists.length > 0 && (
          <Marquee className="text-sm text-muted-foreground">{credit(track.artists)}</Marquee>
        )}
      </div>
      {isPlaying && <Equalizer className="mr-2" />}
      {!current && (
        <Hint label="Remove from queue">
          <Button
            variant="ghost"
            size="icon-sm"
            className="opacity-0 group-hover:opacity-100"
            onClick={() => player.remove(item.key)}
          >
            <XIcon />
          </Button>
        </Hint>
      )}
    </div>
  )
})
