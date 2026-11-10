import type { ComponentProps, ReactNode } from 'react'
import type { PlaybackContext } from '@/bindings'
import { Marquee } from '@/components/marquee'
import { PinBadge } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { useIsPlayingFrom, usePlayer } from '@/lib/player'
import { cn } from '@/lib/utils'

/** Grid card of an album or artist; `link` is a full-card overlay link, the play button sits above it; other props go to the card, for menus and dragging. */
export function MediaCard({
  cover,
  title,
  subtitle,
  context,
  link,
  pinned = false,
  tightSubtitle = false,
  className,
  ...props
}: Omit<ComponentProps<'div'>, 'title'> & {
  cover: ReactNode
  title: string
  subtitle: ReactNode
  context: PlaybackContext
  link: ReactNode
  pinned?: boolean
  /** Pulls the subtitle up against the title, for a short line of counts that reads as one caption with it. */
  tightSubtitle?: boolean
}) {
  const playingHere = useIsPlayingFrom(context)
  const isPlaying = usePlayer((state) => playingHere && state.isPlaying)
  return (
    // off-screen cards skip layout and paint; the browser keeps their last size so the scrollbar stays steady
    <div
      className={cn(
        'group relative flex flex-col gap-2 rounded-md p-3 transition-colors [contain-intrinsic-size:auto_15rem] [content-visibility:auto] hover:bg-surface-hover',
        className,
      )}
      {...props}
    >
      <div className="relative">
        <div className="*:w-full *:shadow-lg *:shadow-black/40">{cover}</div>
        {pinned && <PinBadge size="card" />}
        <PlayButton
          context={context}
          className={cn(
            'absolute right-2 bottom-2 z-10 size-12 translate-y-2 opacity-0 transition-all group-hover:translate-y-0 group-hover:opacity-100',
            isPlaying && 'translate-y-0 opacity-100',
          )}
        />
      </div>
      <Marquee className={cn('font-semibold', playingHere && 'text-primary')}>{title}</Marquee>
      <Marquee className={cn('text-sm text-muted-foreground', tightSubtitle ? '-mt-1.5' : '-mt-1')}>{subtitle}</Marquee>
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
