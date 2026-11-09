import { Link } from '@tanstack/react-router'
import {
  ListMusicIcon,
  LocateFixedIcon,
  MicVocalIcon,
  PauseIcon,
  PlayIcon,
  Repeat1Icon,
  RepeatIcon,
  ShuffleIcon,
  SkipBackIcon,
  SkipForwardIcon,
  Volume1Icon,
  Volume2Icon,
  VolumeIcon,
  VolumeXIcon,
} from 'lucide-react'
import { type ComponentProps, type ReactNode, useRef, useState } from 'react'
import { VOLUME_MAX } from '@/bindings'
import { ArtistLinks } from '@/components/artist-menu'
import { Cover } from '@/components/cover'
import { Hint } from '@/components/hint'
import { Marquee } from '@/components/marquee'
import { TrackMenu } from '@/components/track-menu'
import { Button } from '@/components/ui/button'
import { Slider } from '@/components/ui/slider'
import { formatClock } from '@/lib/format'
import {
  LOCATE_KEYS,
  LYRICS_KEYS,
  NEXT_KEYS,
  PREVIOUS_KEYS,
  MUTE_KEYS,
  REPEAT_KEYS,
  SHUFFLE_KEYS,
  type SidePanel,
  togglePanel,
  useLayout,
} from '@/lib/layout'
import { useLocatePlaying } from '@/lib/locate'
import { currentItem, nextRepeat, player, songsContext, usePlayer } from '@/lib/player'
import { cn } from '@/lib/utils'

/** Bottom bar: the current track, transport controls with the seek bar, then queue and volume. */
export function PlayerBar() {
  return (
    <footer className="grid h-[4.5rem] grid-cols-[minmax(11rem,30%)_minmax(0,40%)_minmax(11rem,30%)] items-center px-2">
      <NowPlaying />
      <Transport />
      <div className="flex items-center justify-end gap-2 pr-2">
        <PanelToggle panel="lyrics" label="Lyrics" keys={LYRICS_KEYS}>
          <MicVocalIcon />
        </PanelToggle>
        <PanelToggle panel="queue" label="Queue">
          <ListMusicIcon />
        </PanelToggle>
        <VolumeControl />
      </div>
    </footer>
  )
}

/** Current track; right-click opens the track menu, or the artist menu over the artist name, and the button beside it
 * shows the track in the list it plays from. */
function NowPlaying() {
  const track = usePlayer((state) => currentItem(state)?.track)
  const locatePlaying = useLocatePlaying()
  // songs added to the queue while nothing played have no view of their own; the library is theirs
  const context = usePlayer((state) => state.context) ?? songsContext
  if (track === undefined) return <div />
  return (
    <TrackMenu track={track} context={context}>
      <div className="flex max-w-full min-w-0 items-center gap-3 justify-self-start pr-6">
        <Cover src={track.cover} kind="track" className="size-14" />
        <div className="flex min-w-0 flex-col">
          <Marquee play="always" className="text-sm font-medium">
            {track.albumId === null ? (
              track.title
            ) : (
              <Link to="/albums/$albumId" params={{ albumId: track.albumId }} className="hover:underline">
                {track.title}
              </Link>
            )}
          </Marquee>
          <ArtistLinks artists={track.artists} play="always" className="text-xs text-muted-foreground" />
        </div>
        <Hint label="Jump to" keys={LOCATE_KEYS}>
          <Button
            variant="ghost"
            size="icon"
            className="text-subtle-foreground hover:bg-foreground/10"
            onClick={locatePlaying}
          >
            <LocateFixedIcon />
          </Button>
        </Hint>
      </div>
    </TrackMenu>
  )
}

/** Accent colour plus a dot under the icon, the way toggled player options are shown. */
const toggledClass =
  'text-primary hover:text-primary-hover after:absolute after:bottom-0 after:size-1 after:rounded-full after:bg-current'

/** Shuffle, previous, play or pause, next and repeat, with the seek bar under them. */
export function Transport() {
  const hasTrack = usePlayer((state) => state.index !== null)
  const isPlaying = usePlayer((state) => state.isPlaying)
  const shuffle = usePlayer((state) => state.shuffle)
  const repeat = usePlayer((state) => state.repeat)
  const RepeatModeIcon = repeat === 'one' ? Repeat1Icon : RepeatIcon
  const PlayPauseIcon = isPlaying ? PauseIcon : PlayIcon

  return (
    <div className="flex flex-col items-center gap-1">
      <div className="flex items-center gap-4">
        <Hint label={shuffle ? 'Disable shuffle' : 'Enable shuffle'} keys={SHUFFLE_KEYS}>
          <Button
            variant="ghost"
            size="icon"
            className={cn('relative', shuffle && toggledClass)}
            onClick={() => player.setShuffle(!shuffle)}
          >
            <ShuffleIcon />
          </Button>
        </Hint>
        <Hint label="Previous" keys={PREVIOUS_KEYS}>
          <Button variant="ghost" size="icon" disabled={!hasTrack} onClick={player.previous}>
            <SkipBackIcon className="fill-current" />
          </Button>
        </Hint>
        <Hint label={isPlaying ? 'Pause' : 'Play'}>
          <Button variant="light" size="icon" disabled={!hasTrack} onClick={player.toggle}>
            <PlayPauseIcon className="fill-current" />
          </Button>
        </Hint>
        <Hint label="Next" keys={NEXT_KEYS}>
          <Button variant="ghost" size="icon" disabled={!hasTrack} onClick={player.next}>
            <SkipForwardIcon className="fill-current" />
          </Button>
        </Hint>
        <Hint
          label={{ off: 'Enable repeat', all: 'Enable repeat one', one: 'Disable repeat' }[repeat]}
          keys={REPEAT_KEYS}
        >
          <Button
            variant="ghost"
            size="icon"
            className={cn('relative', repeat !== 'off' && toggledClass)}
            onClick={() => player.setRepeat(nextRepeat[repeat])}
          >
            <RepeatModeIcon />
          </Button>
        </Hint>
      </div>
      <SeekBar />
    </div>
  )
}

/** Value shown while the user drags a slider, falling back to the backend value otherwise, and whether a drag is going on. */
function useDraft(value: number) {
  const [draft, setDraft] = useState<number | null>(null)
  return [draft ?? value, setDraft, draft !== null] as const
}

/** Single-thumb slider reporting plain numbers; every slide that moved the value ends with `onSlideEnd`, even one that
 * came back to where it started, which Radix does not commit. */
function ValueSlider({
  value,
  onSlide,
  onSlideEnd,
  ...props
}: Omit<
  ComponentProps<typeof Slider>,
  'value' | 'onChange' | 'onValueChange' | 'onValueCommit' | 'onLostPointerCapture'
> & {
  value: number
  onSlide: (value: number) => void
  onSlideEnd: (value: number) => void
}) {
  // set while the value moved and the slide has not ended yet
  const moved = useRef(false)
  const end = (last: number) => {
    moved.current = false
    onSlideEnd(last)
  }
  return (
    <Slider
      value={[value]}
      onValueChange={([next]) => {
        if (next === undefined) return
        moved.current = true
        onSlide(next)
      }}
      onValueCommit={([next]) => next !== undefined && end(next)}
      // the pointer is let go only after Radix commits, so a slide not ended by then is one it left uncommitted
      onLostPointerCapture={() => moved.current && end(value)}
      {...props}
    />
  )
}

function SeekBar() {
  const durationMs = usePlayer((state) => currentItem(state)?.track.durationMs)
  const positionMs = usePlayer((state) => state.positionMs)
  const [shown, setDraft] = useDraft(positionMs)
  const clock = 'w-10 text-xs text-muted-foreground tabular-nums'

  return (
    <div className="flex w-full items-center gap-2">
      <span className={cn(clock, 'text-right')}>{durationMs === undefined ? '-:--' : formatClock(shown)}</span>
      <ValueSlider
        aria-label="Seek"
        disabled={durationMs === undefined}
        max={durationMs ?? 1}
        step={1000}
        value={durationMs === undefined ? 0 : shown}
        onSlide={setDraft}
        onSlideEnd={(position) => {
          setDraft(null)
          // show the new position at once instead of waiting for the next progress event
          usePlayer.setState({ positionMs: position })
          player.seek(position)
        }}
      />
      <span className={clock}>{durationMs === undefined ? '-:--' : formatClock(durationMs)}</span>
    </div>
  )
}

/** Player bar button that opens one of the side panels in place of the other. */
function PanelToggle({
  panel,
  label,
  keys,
  children,
}: {
  panel: SidePanel
  label: string
  keys?: readonly string[]
  children: ReactNode
}) {
  const open = useLayout((state) => state.panel === panel)
  return (
    <Hint label={label} {...(keys && { keys })}>
      <Button
        variant="ghost"
        size="icon"
        className={cn('relative', open && toggledClass)}
        onClick={() => togglePanel(panel)}
      >
        {children}
      </Button>
    </Hint>
  )
}

function volumeIcon(volume: number) {
  if (volume === 0) return VolumeXIcon
  if (volume < VOLUME_MAX / 3) return VolumeIcon
  if (volume < (VOLUME_MAX * 2) / 3) return Volume1Icon
  return Volume2Icon
}

function VolumeControl() {
  const volume = usePlayer((state) => state.volume)
  // muted, the slider keeps the level dimmed, and unmuting goes back to it
  const muted = usePlayer((state) => state.muted)
  const [shown, setDraft, sliding] = useDraft(volume)
  const [hovered, setHovered] = useState(false)
  const Icon = volumeIcon(muted ? 0 : shown)

  return (
    <div className="flex items-center gap-1">
      <Hint label={muted ? 'Unmute' : 'Mute'} keys={MUTE_KEYS}>
        <Button variant="ghost" size="icon" onClick={() => player.setMuted(!muted)}>
          <Icon />
        </Button>
      </Hint>
      <ValueSlider
        aria-label="Volume"
        thumbHint={sliding || hovered ? `${shown}%` : null}
        onPointerEnter={() => setHovered(true)}
        onPointerLeave={() => setHovered(false)}
        className={cn('w-24', muted && '[&_[data-slot=slider-range]]:opacity-40')}
        max={VOLUME_MAX}
        value={shown}
        onSlide={(level) => {
          setDraft(level)
          player.setVolume(level)
        }}
        onSlideEnd={() => setDraft(null)}
      />
    </div>
  )
}
