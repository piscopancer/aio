import { PauseIcon, PlayIcon } from 'lucide-react'
import type { ComponentProps } from 'react'
import type { PlaybackContext, TrackId } from '@/bindings'
import { Button } from '@/components/ui/button'
import { player, useIsPlayingFrom, usePlayer } from '@/lib/player'
import { cn } from '@/lib/utils'

/** Looks of the button: a large raised accent circle for headers and cards, or a small bare icon for list rows. */
const looks = {
  accent: { variant: 'default', size: 'icon-lg', className: 'shadow-lg shadow-black/40' },
  ghost: { variant: 'ghost', size: 'icon', className: 'hover:bg-foreground/10 [&_svg]:size-3.5' },
} as const satisfies Record<string, Pick<ComponentProps<typeof Button>, 'variant' | 'size' | 'className'>>

/** Round button that starts a view, or one song of it with `trackId`, or pauses it when that is what is playing. */
export function PlayButton({
  context,
  trackId,
  look = 'accent',
  disabled,
  className,
}: {
  context: PlaybackContext
  trackId?: TrackId | undefined
  look?: keyof typeof looks
  disabled?: boolean | undefined
  className?: string | undefined
}) {
  const playingHere = useIsPlayingFrom(context, trackId)
  const isPlaying = usePlayer((state) => playingHere && state.isPlaying)
  const Icon = isPlaying ? PauseIcon : PlayIcon
  return (
    <Button
      variant={looks[look].variant}
      size={looks[look].size}
      disabled={disabled}
      aria-label={isPlaying ? 'Pause' : 'Play'}
      className={cn(looks[look].className, className)}
      onClick={(event) => {
        event.stopPropagation()
        player.playOrToggle(context, trackId)
      }}
    >
      <Icon className="fill-current" />
    </Button>
  )
}
