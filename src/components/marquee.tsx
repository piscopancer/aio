import { animate, useReducedMotion } from 'motion/react'
import { type ReactNode, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { cn } from '@/lib/utils'

/** Seconds the text rests at its start before each run, so it can be read from the beginning. */
const REST_SECONDS = 2

/** Pace of the run, in pixels per second. */
const PIXELS_PER_SECOND = 40

/** Whether the text of `box`, or of `text` within it while running, is wider than the box by any fraction of a pixel, as the ellipsis cuts it at any; the measures are fractional, where `scrollWidth` would round a line just too wide down to the box and leave it cut but still. */
const overflows = (box: HTMLElement, text: HTMLElement | null = null) => {
  const range = document.createRange()
  range.selectNodeContents(text ?? box)
  return range.getBoundingClientRect().width > box.getBoundingClientRect().width + 0.01
}

/** When the text runs: all the time, or only while the pointer is over the card or row it belongs to. */
type MarqueePlay = 'always' | 'hover'

/**
 * One line of text that is cut with an ellipsis when it does not fit, and then runs as a marquee instead: it rests at
 * its start, moves left, and a copy of it follows after a gap, so the line keeps going forward round and round rather
 * than bouncing back. With `play="hover"` it runs only while the pointer is over the nearest Tailwind `group`, the card
 * or row whose hover already lights it, and stays cut otherwise; in the reduced motion mode it never runs. `className`
 * styles the line, which is a block as wide as its container.
 */
export function Marquee({
  children,
  play = 'hover',
  className,
}: {
  children: ReactNode
  play?: MarqueePlay | undefined
  className?: string | undefined
}) {
  const boxRef = useRef<HTMLSpanElement>(null)
  const trackRef = useRef<HTMLSpanElement>(null)
  const firstRef = useRef<HTMLSpanElement>(null)
  const reducedMotion = useReducedMotion() ?? false
  const [hovered, setHovered] = useState(false)
  const [overflowing, setOverflowing] = useState(false)
  const active = (play === 'always' || hovered) && !reducedMotion
  const running = active && overflowing

  useEffect(() => {
    const box = boxRef.current
    if (play !== 'hover' || box === null) return undefined
    const host = box.closest('.group') ?? box
    // a line that fits has nothing to run, so hovering it renders nothing
    const enter = () => {
      if (overflows(box)) setHovered(true)
    }
    const leave = () => setHovered(false)
    host.addEventListener('pointerenter', enter)
    host.addEventListener('pointerleave', leave)
    return () => {
      host.removeEventListener('pointerenter', enter)
      host.removeEventListener('pointerleave', leave)
    }
  }, [play])

  // measured only while the line may run, so a long list of resting lines costs nothing; the text is the first copy
  // while running and the box's whole content while cut
  useLayoutEffect(() => {
    const box = boxRef.current
    if (!active || box === null) return undefined
    const measure = () => setOverflowing(overflows(box, firstRef.current))
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(box)
    return () => observer.disconnect()
  }, [active, children])

  useEffect(() => {
    const track = trackRef.current
    const first = firstRef.current
    if (!running || track === null || first === null) return undefined
    const distance = first.offsetWidth + Number.parseFloat(getComputedStyle(track).columnGap)
    const duration = REST_SECONDS + distance / PIXELS_PER_SECOND
    const controls = animate(
      track,
      { x: [0, 0, -distance] },
      { duration, times: [0, REST_SECONDS / duration, 1], ease: 'linear', repeat: Infinity },
    )
    return () => controls.stop()
  }, [running, children])

  return (
    <span
      ref={boxRef}
      className={cn(
        'block overflow-hidden whitespace-nowrap',
        running
          ? 'mask-[linear-gradient(to_right,transparent,black_0.5rem,black_calc(100%-1.5rem),transparent)]'
          : 'text-ellipsis',
        className,
      )}
    >
      {running ? (
        <span ref={trackRef} className="inline-flex gap-12">
          <span ref={firstRef}>{children}</span>
          {/* the copy that follows is only seen, never read out or clicked */}
          <span aria-hidden inert>
            {children}
          </span>
        </span>
      ) : (
        children
      )}
    </span>
  )
}
