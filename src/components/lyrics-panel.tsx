import { useQuery } from '@tanstack/react-query'
import { getCurrentWindow } from '@tauri-apps/api/window'
import {
  ArrowLeftToLineIcon,
  ArrowRightToLineIcon,
  CircleAlertIcon,
  LoaderCircleIcon,
  Maximize2Icon,
  Minimize2Icon,
  XIcon,
} from 'lucide-react'
import { type AnimationPlaybackControls, animate, motion } from 'motion/react'
import { type ComponentProps, type ReactNode, useCallback, useEffect, useRef, useState } from 'react'
import type { LyricLine, LyricsOrigin, Track } from '@/bindings'
import { Cover } from '@/components/cover'
import { CoverGradient } from '@/components/cover-gradient'
import { Hint } from '@/components/hint'
import { Marquee } from '@/components/marquee'
import { Transport } from '@/components/player-bar'
import { Button } from '@/components/ui/button'
import { FULLSCREEN_LYRICS_KEYS, LYRICS_KEYS, type LyricsView, setLyricsView, togglePanel } from '@/lib/layout'
import { currentItem, player, usePlayer } from '@/lib/player'
import { queries } from '@/lib/query'
import { credit } from '@/lib/format'
import { cn } from '@/lib/utils'

/** How long the panel leaves the scroll position to the user after they scroll it themselves. */
const USER_SCROLL_HOLD_MS = 3000

/** Text size of the lyrics in the side panel, expanded over the library, and on the full-screen stage beside the cover. */
const lineText = { panel: 'text-2xl/snug', wide: 'text-4xl/tight', stage: 'text-4xl/tight' } as const
type Size = keyof typeof lineText

/** The centred band the stage lays its cover and lyrics in, as wide as both together. */
const STAGE_BAND = 'mx-auto w-full max-w-6xl px-12'

/** The playing song, whose lyrics are shown on the flowing colours of its cover. */
const usePlayingTrack = () => usePlayer((state) => currentItem(state)?.track)

/** How long the mouse rests before the full-screen lyrics hide their controls and the pointer. */
const IDLE_MS = 3000

/** A round button over the tinted lyrics, with its hint. */
function StageButton({
  label,
  keys,
  onClick,
  children,
}: {
  label: string
  keys?: readonly string[]
  onClick: () => void
  children: ReactNode
}) {
  return (
    <Hint label={label} {...(keys && { keys })}>
      <Button variant="ghost" size="icon" className="hover:bg-foreground/10" onClick={onClick}>
        {children}
      </Button>
    </Hint>
  )
}

/** Buttons that move the lyrics between their views and close them; each view offers the ones leading elsewhere. */
function ViewButtons({ view }: { view: LyricsView }) {
  return (
    <div className="flex items-center">
      {view === 'panel' && (
        <StageButton label="Expand" onClick={() => setLyricsView('expanded')}>
          <ArrowLeftToLineIcon />
        </StageButton>
      )}
      {view === 'expanded' && (
        <StageButton label="Collapse" onClick={() => setLyricsView('panel')}>
          <ArrowRightToLineIcon />
        </StageButton>
      )}
      {view === 'fullscreen' ? (
        <StageButton label="Exit full screen" onClick={() => setLyricsView('panel')}>
          <Minimize2Icon />
        </StageButton>
      ) : (
        <StageButton label="Full screen" keys={FULLSCREEN_LYRICS_KEYS} onClick={() => setLyricsView('fullscreen')}>
          <Maximize2Icon />
        </StageButton>
      )}
      <StageButton label="Close" keys={LYRICS_KEYS} onClick={() => togglePanel('lyrics')}>
        <XIcon />
      </StageButton>
    </div>
  )
}

/** Right panel with the lyrics of the playing song, following the song line by line when they are timed. */
export function LyricsPanel() {
  const track = usePlayingTrack()

  return (
    <aside className="relative isolate flex w-105 shrink-0 flex-col overflow-hidden rounded-lg">
      <CoverGradient cover={track?.cover ?? null} />
      <header className="flex items-center justify-between px-4 py-3">
        <h2 className="font-bold">Lyrics</h2>
        <ViewButtons view="panel" />
      </header>
      {track === undefined ? (
        <LyricsArea size="panel">
          <Message>Nothing is playing</Message>
        </LyricsArea>
      ) : (
        <TrackLyrics key={track.id} track={track} size="panel" />
      )}
    </aside>
  )
}

/** Puts the window into full screen while mounted and takes it out when gone, even if F11 had put it there before. */
function useWindowFullscreen() {
  useEffect(() => {
    const window = getCurrentWindow()
    void window.setFullscreen(true)
    return () => void window.setFullscreen(false)
  }, [])
}

/** Whether the mouse moved within the last few seconds over the element the handler is put on. */
function useAwake() {
  const [awake, setAwake] = useState(true)
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined)
  const wake = useCallback(() => {
    setAwake(true)
    clearTimeout(timer.current)
    timer.current = setTimeout(() => setAwake(false), IDLE_MS)
  }, [])
  useEffect(() => {
    wake()
    return () => clearTimeout(timer.current)
  }, [wake])
  return { awake, wake }
}

/** Large lyrics beside a large cover of the song: over the library and panel area, or over the whole screen with the player's controls showing while the mouse moves; Escape goes back to the panel. */
export function LyricsStage({ view }: { view: Exclude<LyricsView, 'panel'> }) {
  return view === 'fullscreen' ? (
    <FullscreenStage />
  ) : (
    <Stage view={view} className="absolute inset-0 z-30 rounded-lg" />
  )
}

function FullscreenStage() {
  useWindowFullscreen()
  const { awake, wake } = useAwake()
  return (
    <Stage
      view="fullscreen"
      awake={awake}
      onMouseMove={wake}
      className={cn('fixed inset-0 z-50', !awake && 'cursor-none')}
    >
      <div
        className={cn(
          'absolute inset-x-0 bottom-0 z-10 flex justify-center pt-12 pb-6 transition-opacity duration-300',
          !awake && 'pointer-events-none opacity-0',
        )}
      >
        <div className="w-full max-w-2xl">
          <Transport />
        </div>
      </div>
    </Stage>
  )
}

function Stage({
  view,
  awake = true,
  onMouseMove,
  className,
  children,
}: {
  view: Exclude<LyricsView, 'panel'>
  awake?: boolean
  onMouseMove?: () => void
  className: string
  children?: ReactNode
}) {
  const track = usePlayingTrack()
  // the cover and title stand beside the lyrics only on the full screen, as the player bar shows them otherwise
  const size = view === 'fullscreen' ? 'stage' : 'wide'

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setLyricsView('panel')
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  return (
    <div onMouseMove={onMouseMove} className={cn('isolate flex overflow-hidden', className)}>
      <CoverGradient cover={track?.cover ?? null} />
      <div
        className={cn(
          'absolute top-3 right-3 z-10 transition-opacity duration-300',
          !awake && 'pointer-events-none opacity-0',
        )}
      >
        <ViewButtons view={view} />
      </div>
      {track === undefined ? (
        <LyricsArea size={size}>
          <Message>Nothing is playing</Message>
        </LyricsArea>
      ) : (
        <>
          <TrackLyrics key={track.id} track={track} size={size} />
          {/* the cover lies over the scrolling lyrics, so their scroll bar can run along the stage's edge */}
          {size === 'stage' && (
            <div className="pointer-events-none absolute inset-0 flex items-center">
              <div className={STAGE_BAND}>
                <div className="pointer-events-auto flex w-2/5 flex-col gap-6">
                  <Cover src={track.cover} kind="track" className="aspect-square size-auto w-full shadow-2xl" />
                  <div className="flex min-w-0 flex-col gap-1">
                    <Marquee play="always" className="text-3xl font-bold">
                      {track.title}
                    </Marquee>
                    {track.artists.length > 0 && (
                      <Marquee play="always" className="text-xl text-foreground/70">
                        {credit(track.artists)}
                      </Marquee>
                    )}
                  </div>
                </div>
              </div>
            </div>
          )}
        </>
      )}
      {children}
    </div>
  )
}

/** Where the lyrics go: the whole panel below its header, or the stage's column beside the cover with the scroll bar at the stage's edge; text that does not scroll sits in the middle. */
function LyricsArea({
  size,
  scrolls = false,
  children,
  ...props
}: Omit<ComponentProps<'div'>, 'className'> & { size: Size; scrolls?: boolean }) {
  const place = size === 'panel' ? 'relative min-h-0 flex-1 px-6' : 'absolute inset-0'
  return (
    <div className={cn(place, scrolls ? 'overflow-y-auto' : 'flex items-center')} {...props}>
      {size === 'panel' && children}
      {size === 'wide' && <div className="mx-auto w-full max-w-4xl px-12">{children}</div>}
      {size === 'stage' && (
        <div className={STAGE_BAND}>
          <div className="ml-[calc(40%+4rem)]">{children}</div>
        </div>
      )}
    </div>
  )
}

function TrackLyrics({ track, size }: { track: Track; size: Size }) {
  const lyrics = useQuery(queries.lyrics(track.id))
  const message = (icon: ReactNode, text: string) => (
    <LyricsArea size={size}>
      <Message icon={icon}>{text}</Message>
    </LyricsArea>
  )
  if (lyrics.error !== null) return message(<CircleAlertIcon className="text-destructive" />, lyrics.error.message)
  const { data } = lyrics
  if (data === undefined) return message(<LoaderCircleIcon className="animate-spin" />, 'Looking for lyrics…')
  switch (data.kind) {
    case 'missing':
      return message(null, 'No lyrics found for this song')
    case 'instrumental':
      return message(null, '♪ Instrumental ♪')
    case 'plain':
      return (
        <LyricsArea size={size} scrolls>
          <p className={cn(lineText[size], 'py-6 font-bold whitespace-pre-line select-text')}>{data.text}</p>
          <Credit origin={data.origin} />
        </LyricsArea>
      )
    case 'synced':
      return <SyncedLyrics lines={data.lines} origin={data.origin} size={size} />
    default:
      return data satisfies never
  }
}

/** Index of the line being sung at `ms`, or -1 before the first one. */
const lineAt = (lines: readonly LyricLine[], ms: number) => lines.findLastIndex((line) => line.startMs <= ms)

/** Reads the playback position in milliseconds, moved on smoothly between the player's quarter-second reports. */
function usePlaybackClock() {
  const { positionMs, isPlaying } = usePlayer.getState()
  const base = useRef({ positionMs, isPlaying, at: performance.now() })
  useEffect(() => {
    const mark = ({ positionMs, isPlaying }: { positionMs: number; isPlaying: boolean }) => {
      base.current = { positionMs, isPlaying, at: performance.now() }
    }
    mark(usePlayer.getState())
    return usePlayer.subscribe((state, before) => {
      if (state.positionMs !== before.positionMs || state.isPlaying !== before.isPlaying) mark(state)
    })
  }, [])
  return useCallback(() => {
    const { positionMs, isPlaying, at } = base.current
    return isPlaying ? positionMs + performance.now() - at : positionMs
  }, [])
}

function SyncedLyrics({ lines, origin, size }: { lines: LyricLine[]; origin: LyricsOrigin; size: Size }) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const scrolling = useRef<AnimationPlaybackControls | null>(null)
  const userScrolledAt = useRef(-Infinity)
  const clock = usePlaybackClock()
  const [active, setActive] = useState(() => lineAt(lines, clock()))

  // one frame loop finds the sung line and lights the sung words of it, without rendering for the words
  useEffect(() => {
    let frame = 0
    const tick = () => {
      const ms = clock()
      setActive(lineAt(lines, ms))
      for (const word of scrollRef.current?.querySelectorAll<HTMLElement>('[data-current] [data-start]') ?? [])
        word.toggleAttribute('data-sung', Number(word.dataset['start']) <= ms)
      frame = requestAnimationFrame(tick)
    }
    frame = requestAnimationFrame(tick)
    return () => cancelAnimationFrame(frame)
  }, [lines, clock])

  const first = useRef(true)
  useEffect(() => {
    const container = scrollRef.current
    if (container === null || performance.now() - userScrolledAt.current < USER_SCROLL_HOLD_MS) return
    const line = container.querySelector<HTMLElement>(`[data-index="${Math.max(active, 0)}"]`)
    if (line === null) return
    const target = Math.max(0, line.offsetTop + line.offsetHeight / 2 - container.clientHeight / 2)
    scrolling.current?.stop()
    if (first.current) {
      first.current = false
      container.scrollTop = target
      return
    }
    scrolling.current = animate(container.scrollTop, target, {
      type: 'spring',
      stiffness: 90,
      damping: 20,
      onUpdate: (top) => {
        container.scrollTop = top
      },
    })
  }, [active])

  const holdScroll = () => {
    userScrolledAt.current = performance.now()
    scrolling.current?.stop()
  }

  return (
    <LyricsArea size={size} scrolls ref={scrollRef} onWheel={holdScroll} onTouchMove={holdScroll}>
      <div className="flex flex-col gap-4 py-[50vh]">
        {lines.map((line, index) => (
          <SungLine
            // the same line can be sung twice, so its place keys it
            key={index}
            line={line}
            index={index}
            distance={index - active}
            size={size}
          />
        ))}
        <Credit origin={origin} />
      </div>
    </LyricsArea>
  )
}

function SungLine({ line, index, distance, size }: { line: LyricLine; index: number; distance: number; size: Size }) {
  const current = distance === 0
  const away = Math.min(Math.abs(distance), 4)
  return (
    <motion.p
      data-index={index}
      data-current={current || undefined}
      initial={false}
      animate={{
        opacity: current ? 1 : distance < 0 ? 0.35 : 0.5,
        scale: current ? 1 : 0.96,
        filter: `blur(${current ? 0 : away * 0.5}px)`,
      }}
      transition={{ type: 'spring', stiffness: 200, damping: 26 }}
      onClick={() => {
        // show the new position at once instead of waiting for the next progress event
        usePlayer.setState({ positionMs: line.startMs })
        player.seek(line.startMs)
      }}
      className={cn(lineText[size], 'origin-left cursor-pointer font-bold hover:opacity-100!')}
    >
      {line.text === '' ? (
        <span className="tracking-[0.4em]">♪</span>
      ) : line.words.length === 0 ? (
        line.text
      ) : (
        line.words.map((word, at) => (
          <span
            // words repeat within a line, so their place keys them
            key={at}
            data-start={word.startMs}
            className={cn(current && 'text-foreground/45 transition-colors duration-150 data-sung:text-foreground')}
          >
            {word.text}
          </span>
        ))
      )}
    </motion.p>
  )
}

function Credit({ origin }: { origin: LyricsOrigin }) {
  if (origin.kind !== 'lrclib') return null
  return <p className="pt-8 pb-6 text-xs text-foreground/50">Lyrics from LRCLIB</p>
}

function Message({ icon, children }: { icon?: ReactNode; children: ReactNode }) {
  return (
    <p className="flex w-full items-center justify-center gap-2 pb-12 text-center text-sm text-foreground/70 [&_svg]:size-4 [&_svg]:shrink-0">
      {icon}
      {children}
    </p>
  )
}
