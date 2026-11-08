import type { ReactNode } from 'react'
import type { PlaybackContext } from '@/bindings'
import { PlayButton } from '@/components/play-button'
import { useIsPlayingFrom, usePlayer } from '@/lib/player'
import { cn } from '@/lib/utils'

/** Grid card of an album or artist; `link` is a full-card overlay link, the play button sits above it. */
export function MediaCard({
  cover,
  title,
  subtitle,
  context,
  link,
}: {
  cover: ReactNode
  title: string
  subtitle: ReactNode
  context: PlaybackContext
  link: ReactNode
}) {
  const playingHere = useIsPlayingFrom(context)
  const isPlaying = usePlayer((state) => playingHere && state.isPlaying)
  return (
    <div className="group relative flex flex-col gap-2 rounded-md p-3 transition-colors hover:bg-surface-hover">
      <div className="relative">
        <div className="shadow-lg shadow-black/40 [&>*]:w-full">{cover}</div>
        <PlayButton
          context={context}
          className={cn(
            'absolute right-2 bottom-2 z-10 size-12 translate-y-2 opacity-0 transition-all group-hover:translate-y-0 group-hover:opacity-100',
            isPlaying && 'translate-y-0 opacity-100',
          )}
        />
      </div>
      <span className={cn('truncate font-semibold', playingHere && 'text-primary')}>{title}</span>
      <span className="-mt-1 truncate text-sm text-muted-foreground">{subtitle}</span>
      {link}
    </div>
  )
}

/** Responsive card grid of the main view. */
export function MediaGrid({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-[repeat(auto-fill,minmax(10.5rem,1fr))] gap-1 px-3">{children}</div>
}

/** Full-card link layer of a `MediaCard`. */
export const cardLinkClass = 'absolute inset-0 rounded-md'
