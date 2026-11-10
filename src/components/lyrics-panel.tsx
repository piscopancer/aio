import { useMutation, useQuery } from '@tanstack/react-query'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import {
  ArrowLeftToLineIcon,
  ArrowRightToLineIcon,
  CheckIcon,
  CircleAlertIcon,
  CopyIcon,
  LoaderCircleIcon,
  Maximize2Icon,
  Minimize2Icon,
  SaveIcon,
  XIcon,
} from 'lucide-react'
import { type AnimationPlaybackControls, animate, motion } from 'motion/react'
import {
  type CSSProperties,
  type ComponentProps,
  type ReactNode,
  memo,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'
import type { LyricsView as FoundLyrics, LyricLine, LyricsOrigin, Track, TrackId } from '@/bindings'
import { Cover } from '@/components/cover'
import { CoverGradient } from '@/components/cover-gradient'
import { Hint } from '@/components/hint'
import { Marquee } from '@/components/marquee'
import { Transport } from '@/components/player-bar'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { ScrollArea } from '@/components/ui/scroll-area'
import { reportError } from '@/lib/errors'
import {
  FULLSCREEN_LYRICS_KEYS,
  LYRICS_KEYS,
  type LyricsView,
  setLyricsView,
  togglePanel,
  useLayout,
} from '@/lib/layout'
import { currentItem, player, usePlayer } from '@/lib/player'
import { mutations, queries, queryClient } from '@/lib/query'
import { credit } from '@/lib/format'
import { cn } from '@/lib/utils'
import { create } from 'zustand'

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

/** How long the copy button shows that the lyrics were copied. */
const COPIED_MS = 2000

/** Lyrics that have words to copy or save, timed or not. */
type TextLyrics = Extract<FoundLyrics, { text: string }>

const hasText = (lyrics: FoundLyrics | null | undefined): lyrics is TextLyrics =>
  lyrics !== undefined && lyrics !== null && 'text' in lyrics

/** The song whose lyrics show as LRCLIB has them rather than as its file holds them, if any. */
const useLrclibChoice = create<{ track: TrackId | null }>()(() => ({ track: null }))

/** The lyrics of a song as its file or LRCLIB gives them, what LRCLIB has when it differs from the file's, and which of the two the panel shows. */
function useShownLyrics(track: TrackId) {
  const lyrics = useQuery(queries.lyrics(track))
  // LRCLIB is asked again only about words the file had before Aio looked
  const fromFile = hasText(lyrics.data) && lyrics.data.origin.kind === 'file'
  const { data } = useQuery({ ...queries.lrclibLyrics(track), enabled: fromFile })
  // a disabled query keeps what it had, which no longer stands once LRCLIB's words are in the file
  const lrclib = (fromFile && data) || null
  const showsLrclib = useLrclibChoice((state) => state.track === track) && lrclib !== null
  return { lyrics, lrclib, shown: showsLrclib ? lrclib : lyrics.data }
}

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
      <header className="flex items-center justify-between gap-2 px-4 py-3">
        <div className="flex min-w-0 items-center gap-2">
          <h2 className="font-bold">Lyrics</h2>
          {track !== undefined && <LyricsActions track={track} />}
        </div>
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
      // a dialog over the stage closes on its own Escape, marking the key as taken
      if (event.key === 'Escape' && !event.defaultPrevented) setLyricsView('panel')
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [])

  return (
    <div onMouseMove={onMouseMove} className={cn('isolate flex overflow-hidden', className)}>
      <CoverGradient cover={track?.cover ?? null} />
      <div
        className={cn(
          'absolute top-3 right-3 z-10 flex items-center gap-2 transition-opacity duration-300',
          !awake && 'pointer-events-none opacity-0',
        )}
      >
        {track !== undefined && <LyricsActions track={track} />}
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

/** Where the lyrics go: the whole panel below its header, or the stage's column beside the cover with the scroll bar at the stage's edge; text that does not scroll sits in the middle. `ref` gets the element that scrolls. */
function LyricsArea({
  size,
  scrolls = false,
  hideScrollbar = false,
  ref,
  children,
  ...handlers
}: Pick<ComponentProps<'div'>, 'ref' | 'onWheel' | 'onTouchMove' | 'children'> & {
  size: Size
  scrolls?: boolean
  /** Hides the scroll bar while the lyrics scroll by themselves. */
  hideScrollbar?: boolean
}) {
  const place = size === 'panel' ? 'relative min-h-0 flex-1' : 'absolute inset-0'
  const column = (
    <>
      {size === 'panel' && <div className="w-full px-6">{children}</div>}
      {size === 'wide' && <div className="mx-auto w-full max-w-4xl px-12">{children}</div>}
      {size === 'stage' && (
        <div className={STAGE_BAND}>
          <div className="ml-[calc(40%+4rem)]">{children}</div>
        </div>
      )}
    </>
  )
  // the scroll area sits inside its place, as Radix pins its root to `position: relative`, which would undo `absolute`
  return scrolls ? (
    <div className={place} {...handlers}>
      <ScrollArea viewportRef={ref} hideScrollbar={hideScrollbar} className="size-full">
        {column}
      </ScrollArea>
    </div>
  ) : (
    <div ref={ref} className={cn(place, 'flex items-center')} {...handlers}>
      {column}
    </div>
  )
}

function TrackLyrics({ track, size }: { track: Track; size: Size }) {
  const { lyrics, shown: data } = useShownLyrics(track.id)
  const message = (icon: ReactNode, text: string) => (
    <LyricsArea size={size}>
      <Message icon={icon}>{text}</Message>
    </LyricsArea>
  )
  if (lyrics.error !== null) return message(<CircleAlertIcon className="text-destructive" />, lyrics.error.message)
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
      // the other version starts afresh, following the song from its own lines
      return <SyncedLyrics key={data.origin.kind} lines={data.lines} origin={data.origin} size={size} />
    default:
      return data satisfies never
  }
}

/** Moves playback to `ms`, showing the new position at once instead of waiting for the next progress event. */
function seekTo(ms: number) {
  usePlayer.setState({ positionMs: ms })
  player.seek(ms)
}

/** Which way each arrow key moves through the lines. */
const LINE_STEPS = { ArrowLeft: -1, ArrowRight: 1 } as const

const isStepKey = (key: string): key is keyof typeof LINE_STEPS => key in LINE_STEPS

/** Lines away from the sung one at which the blur stops growing, so lines farther off look the same. */
const LOOK_DISTANCE = 4

/** Index of the line being sung at `ms`, or -1 before the first one. */
const lineAt = (lines: readonly LyricLine[], ms: number) => lines.findLastIndex((line) => line.startMs <= ms)

/** The sung line without word stamps fills white from the top down to `--fill`, from 0 to 1, over the dimmed rest as the unsung words of a stamped line are, glowing the more it fills. */
const FILLING = {
  backgroundImage:
    'linear-gradient(to bottom, var(--foreground) calc(var(--fill, 0) * 100% - 0.15em), color-mix(in oklab, var(--foreground) 45%, transparent) calc(var(--fill, 0) * 100% + 0.15em))',
  backgroundClip: 'text',
  color: 'transparent',
  filter:
    'drop-shadow(0 0 calc(var(--fill, 0) * 0.3em) color-mix(in oklab, var(--foreground) calc(var(--fill, 0) * 70%), transparent))',
} satisfies CSSProperties

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

  // one frame loop finds the sung line and lights the sung words of it, or fills it when it has none, without rendering for either
  useEffect(() => {
    let frame = 0
    const tick = () => {
      const ms = clock()
      const index = lineAt(lines, ms)
      setActive(index)
      for (const word of scrollRef.current?.querySelectorAll<HTMLElement>('[data-current] [data-start]') ?? [])
        word.toggleAttribute('data-sung', Number(word.dataset['start']) <= ms)
      const filling = scrollRef.current?.querySelector<HTMLElement>('[data-current] [data-fill]')
      const line = lines[index]
      if (filling != null && line !== undefined) {
        // the line is sung until the next one starts, the last one until the song ends
        const end = lines[index + 1]?.startMs ?? currentItem(usePlayer.getState())?.track.durationMs ?? line.startMs
        const progress = Math.min(Math.max((ms - line.startMs) / Math.max(end - line.startMs, 1), 0), 1)
        filling.style.setProperty('--fill', String(progress))
      }
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

  // the scroll bar shows only while the user steers, not while the lyrics follow the song
  const [steering, setSteering] = useState(false)
  const steeringTimer = useRef<ReturnType<typeof setTimeout>>(undefined)
  useEffect(() => () => clearTimeout(steeringTimer.current), [])

  // the arrow keys jump to the line before or after the sung one; only the lyrics in front listen, as the panel stays mounted behind the stage
  const view = useLayout((state) => state.lyricsView)
  const inFront = (size === 'panel') === (view === 'panel')
  useEffect(() => {
    if (!inFront) return
    const onKeyDown = (event: KeyboardEvent) => {
      // a focused field, slider or menu handles its own arrows first
      if (event.defaultPrevented || event.altKey || event.ctrlKey || event.shiftKey || event.metaKey) return
      if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) return
      if (!isStepKey(event.key)) return
      const step = LINE_STEPS[event.key]
      const line = lines[Math.min(Math.max(active + step, 0), lines.length - 1)]
      if (line === undefined) return
      event.preventDefault()
      // a jump asked for by key is followed at once, even right after scrolling by hand
      userScrolledAt.current = -Infinity
      seekTo(line.startMs)
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [inFront, lines, active])

  const holdScroll = () => {
    userScrolledAt.current = performance.now()
    scrolling.current?.stop()
    setSteering(true)
    clearTimeout(steeringTimer.current)
    steeringTimer.current = setTimeout(() => setSteering(false), USER_SCROLL_HOLD_MS)
  }

  return (
    <LyricsArea
      size={size}
      scrolls
      hideScrollbar={!steering}
      ref={scrollRef}
      onWheel={holdScroll}
      onTouchMove={holdScroll}
    >
      <div className="flex flex-col gap-4 py-[50vh]">
        {lines.map((line, index) => (
          <SungLine
            // the same line can be sung twice, so its place keys it
            key={index}
            line={line}
            index={index}
            distance={Math.min(Math.max(index - active, -LOOK_DISTANCE), LOOK_DISTANCE)}
            size={size}
          />
        ))}
        <Credit origin={origin} />
      </div>
    </LyricsArea>
  )
}

/** A line of timed lyrics, dimmed and blurred by its distance from the sung one; memoized, so a new sung line renders only the lines whose look changes rather than the whole song. */
const SungLine = memo(function SungLine({
  line,
  index,
  distance,
  size,
}: {
  line: LyricLine
  index: number
  /** Lines from the sung one, cut at `LOOK_DISTANCE` either way, beyond which every line looks the same. */
  distance: number
  size: Size
}) {
  const current = distance === 0
  const away = Math.abs(distance)
  return (
    <motion.p
      data-index={index}
      data-current={current || undefined}
      initial={false}
      animate={{
        // the lines already sung stay whiter than those still to come, as if the fill had left its trace on them
        opacity: current ? 1 : distance < 0 ? 0.65 : 0.35,
        // the sung line stands about a fifth larger than the rest
        scale: current ? 1 : 0.83,
        filter: `blur(${current ? 0 : away * 0.5}px)`,
      }}
      transition={{ type: 'spring', stiffness: 200, damping: 26 }}
      onClick={() => seekTo(line.startMs)}
      className={cn(lineText[size], 'origin-left cursor-pointer font-bold hover:opacity-100!')}
    >
      {line.text === '' ? (
        <span className="tracking-[0.4em]">♪</span>
      ) : line.words.length === 0 ? (
        current ? (
          <span data-fill className="inline-block" style={FILLING}>
            {line.text}
          </span>
        ) : (
          line.text
        )
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
})

/** Switches between the file's lyrics and the different ones LRCLIB has, saves LRCLIB's into the file, and copies the lyrics shown. */
function LyricsActions({ track }: { track: Track }) {
  const { lrclib, shown } = useShownLyrics(track.id)
  if (!hasText(shown)) return null
  return (
    <div className="flex items-center gap-1">
      {lrclib !== null && (
        <Tabs
          value={shown === lrclib ? 'lrclib' : 'file'}
          onValueChange={(source) => useLrclibChoice.setState({ track: source === 'lrclib' ? track.id : null })}
        >
          <TabsList className="gap-0.5 rounded-md bg-foreground/10 p-0.5">
            {LYRICS_SOURCES.map(({ value, label }) => (
              <TabsTrigger
                key={value}
                value={value}
                className="rounded-sm px-2 py-0.5 text-xs text-foreground/60 data-[state=active]:bg-foreground/15"
              >
                {label}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
      )}
      {shown === lrclib && <SaveLyrics track={track} lyrics={lrclib} />}
      <CopyLyrics text={shown.text} />
    </div>
  )
}

/** The two versions of a song's lyrics the switch moves between. */
const LYRICS_SOURCES = [
  { value: 'file', label: 'File' },
  { value: 'lrclib', label: 'LRCLIB' },
] as const

/** Writes LRCLIB's lyrics into the song file over its own, once the user confirms. */
function SaveLyrics({ track, lyrics }: { track: Track; lyrics: TextLyrics }) {
  const [open, setOpen] = useState(false)
  const save = useMutation({
    ...mutations.saveLyrics(),
    onSuccess: (_, { id }) => {
      // the file now holds these very words, so they stay on screen while the file is read again
      queryClient.setQueryData(queries.lyrics(id).queryKey, lyrics)
      useLrclibChoice.setState({ track: null })
    },
  })
  return (
    <>
      <StageButton label="Save to the song file" onClick={() => setOpen(true)}>
        <SaveIcon />
      </StageButton>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Overwrite lyrics?</DialogTitle>
          </DialogHeader>
          <DialogBody>
            <DialogDescription>
              The lyrics in the file of <span className="font-semibold text-foreground">{track.title}</span> are
              replaced with the ones from LRCLIB.
            </DialogDescription>
          </DialogBody>
          <DialogFooter>
            <DialogClose asChild>
              <Button variant="ghost">Cancel</Button>
            </DialogClose>
            <Button
              variant="light"
              disabled={save.isPending}
              onClick={() => save.mutate({ id: track.id, text: lyrics.text, origin: lyrics.origin })}
            >
              Overwrite
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}

/** Copies the lyrics as they are kept, with their time stamps when timed, and shows a check mark for a moment after. */
function CopyLyrics({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  useEffect(() => {
    if (!copied) return
    const timer = setTimeout(() => setCopied(false), COPIED_MS)
    return () => clearTimeout(timer)
  }, [copied])
  return (
    <StageButton
      label={copied ? 'Copied' : 'Copy lyrics'}
      onClick={() =>
        void writeText(text).then(
          () => setCopied(true),
          (error: unknown) => reportError(error, 'Copying the lyrics'),
        )
      }
    >
      {copied ? <CheckIcon /> : <CopyIcon />}
    </StageButton>
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
