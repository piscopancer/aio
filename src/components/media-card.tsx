import { ChevronLeftIcon, ChevronRightIcon } from 'lucide-react'
import { type ComponentProps, type ReactNode, useEffect, useRef, useState } from 'react'
import type { PlaybackContext, TrackId } from '@/bindings'
import { Marquee } from '@/components/marquee'
import { PinBadge } from '@/components/pin-controls'
import { PlayButton } from '@/components/play-button'
import { Button } from '@/components/ui/button'
import { useIsPlayingFrom, usePlayer } from '@/lib/player'
import { cn } from '@/lib/utils'

/** Grid card of an album, artist, playlist or, with `trackId`, of one song played from `context`; `link` is a
 * full-card overlay link or button, the play button sits above it; other props go to the card, for menus and dragging. */
export function MediaCard({
  cover,
  title,
  subtitle,
  context,
  trackId,
  link,
  pinned = false,
  className,
  ...props
}: Omit<ComponentProps<'div'>, 'title'> & {
  cover: ReactNode
  title: string
  subtitle: ReactNode
  context: PlaybackContext
  trackId?: TrackId | undefined
  link: ReactNode
  pinned?: boolean
}) {
  const playingHere = useIsPlayingFrom(context, trackId)
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
          trackId={trackId}
          className={cn(
            'absolute right-2 bottom-2 z-10 size-12 translate-y-2 opacity-0 transition-all group-hover:translate-y-0 group-hover:opacity-100',
            isPlaying && 'translate-y-0 opacity-100',
          )}
        />
      </div>
      <Marquee className={cn('font-semibold', playingHere && 'text-primary')}>{title}</Marquee>
      <Marquee className="-mt-2 text-sm text-muted-foreground">{subtitle}</Marquee>
      {link}
    </div>
  )
}

/** Responsive card grid of the main view. */
export function MediaGrid({ children }: { children: ReactNode }) {
  return <div className="grid grid-cols-[repeat(auto-fill,minmax(10.5rem,1fr))] gap-1 px-3">{children}</div>
}

/** How many cards one arrow of a `CardRow` moves it by. */
const CARDS_PER_CLICK = 3

/** Ways a `CardRow` scrolls, with the arrow of each. */
const PAGES = [
  // the two arrows join into one pill, only slightly rounded where they meet; the outer ends are rounded to half the
  // height rather than fully, as an endless radius would scale the inner one down to nothing
  { direction: -1, label: 'Scroll left', icon: ChevronLeftIcon, rounding: 'rounded-l-2xl rounded-r-md' },
  { direction: 1, label: 'Scroll right', icon: ChevronRightIcon, rounding: 'rounded-l-md rounded-r-2xl' },
] as const

/** How far the ends of a `CardRow` fade out at most, in pixels. */
const ROW_FADE_PX = 200

/** How far a `CardRow` scrolls away from an end before that end fades out fully, in pixels; nearer, it fades less, so it
 * shows sharp once nothing is left that way. */
const ROW_FADE_RAMP_PX = 320

/** Eases in and out, so a fade has no visible start or end. */
const smoothstep = (t: number) => t * t * (3 - 2 * t)

/** How many steps a faded end is drawn in; a plain gradient would start and stop with a visible edge. */
const FADE_STEPS = 12

/** The opacity of a faded end from the edge inwards, eased, as gradient stops spread over `length`. */
const fadeStops = (length: string, toward: 'start' | 'end') =>
  Array.from({ length: FADE_STEPS + 1 }, (_, step) => {
    const t = step / FADE_STEPS
    const offset = toward === 'start' ? `calc(${length} * ${t})` : `calc(100% - ${length} * ${1 - t})`
    const opacity = smoothstep(toward === 'start' ? t : 1 - t)
    return `rgb(0 0 0 / ${opacity.toFixed(3)}) ${offset}`
  }).join(', ')

/** Fades the ends of a `CardRow` out over the lengths its scroll sets. */
const rowFadeMask = `linear-gradient(to right, ${fadeStops('var(--fade-before)', 'start')}, ${fadeStops('var(--fade-after)', 'end')})`

/** How long an end fades over when the row is `distance` away from it. */
const fadeLength = (distance: number) => ROW_FADE_PX * smoothstep(Math.min(distance / ROW_FADE_RAMP_PX, 1))

/** One row of cards that scrolls sideways under its `header`, each card as wide as the narrowest of a `MediaGrid`; its
 * ends fade out while there is more that way, and two arrows at the far end of the header move it by `CARDS_PER_CLICK` cards. */
export function CardRow({ header, children }: { header: ReactNode; children: ReactNode }) {
  const viewport = useRef<HTMLDivElement>(null)
  const track = useRef<HTMLDivElement>(null)
  // whether there is more to see before and after what is shown
  const [more, setMore] = useState({ [-1]: false, [1]: false })

  useEffect(() => {
    const row = viewport.current
    if (row === null || track.current === null) return
    const measure = () => {
      const before = Math.max(0, row.scrollLeft)
      const after = Math.max(0, row.scrollWidth - row.clientWidth - row.scrollLeft)
      // set on the element itself, so scrolling renders nothing
      row.style.setProperty('--fade-before', `${fadeLength(before)}px`)
      row.style.setProperty('--fade-after', `${fadeLength(after)}px`)
      const next = { [-1]: before > 1, [1]: after > 1 }
      setMore((current) => (current[-1] === next[-1] && current[1] === next[1] ? current : next))
    }
    measure()
    // the window resizing changes what fits, the cards coming and going changes what there is
    const observer = new ResizeObserver(measure)
    observer.observe(row)
    observer.observe(track.current)
    row.addEventListener('scroll', measure, { passive: true })
    return () => {
      observer.disconnect()
      row.removeEventListener('scroll', measure)
    }
  }, [])

  return (
    <div>
      <div className="flex items-center justify-between gap-4 px-6 pb-4">
        {header}
        <div className="flex shrink-0 gap-0.5">
          {PAGES.map(({ direction, label, icon: Icon, rounding }) => (
            <Button
              key={direction}
              variant="ghost"
              size="icon"
              aria-label={label}
              disabled={!more[direction]}
              className={cn('w-12 bg-foreground/10 text-foreground hover:bg-foreground/15', rounding)}
              onClick={() => {
                const [first, second] = track.current?.children ?? []
                if (!(first instanceof HTMLElement)) return
                // from one card to the next, gap included
                const card = second instanceof HTMLElement ? second.offsetLeft - first.offsetLeft : first.offsetWidth
                viewport.current?.scrollBy({ left: direction * card * CARDS_PER_CLICK, behavior: 'smooth' })
              }}
            >
              <Icon className="size-5" />
            </Button>
          ))}
        </div>
      </div>
      {/* scrolled by the arrows, the wheel and the touchpad, with no bar */}
      <div
        ref={viewport}
        style={{ maskImage: rowFadeMask }}
        className="snap-x snap-mandatory scroll-px-3 scrollbar-none overflow-x-auto"
      >
        <div ref={track} className="grid w-max auto-cols-42 grid-flow-col gap-1 px-3 *:snap-start">
          {children}
        </div>
      </div>
    </div>
  )
}

/** Full-card link layer of a `MediaCard`. */
export const cardLinkClass = 'absolute inset-0 rounded-md'
