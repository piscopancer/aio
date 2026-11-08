import { PauseIcon, PlayIcon } from 'lucide-react'
import type { PlaybackContext } from '@/bindings'
import { Button } from '@/components/ui/button'
import { player, useIsPlayingFrom, usePlayer } from '@/lib/player'
import { cn } from '@/lib/utils'

/** Round accent button that starts a view or pauses it when the view is what is playing. */
export function PlayButton({
  context,
  disabled,
  className,
}: {
  context: PlaybackContext
  disabled?: boolean | undefined
  className?: string | undefined
}) {
  const playingHere = useIsPlayingFrom(context)
  const isPlaying = usePlayer((state) => playingHere && state.isPlaying)
  const Icon = isPlaying ? PauseIcon : PlayIcon
  return (
    <Button
      size="icon-lg"
      disabled={disabled}
      aria-label={isPlaying ? 'Pause' : 'Play'}
      className={cn('shadow-lg shadow-black/40', className)}
      onClick={(event) => {
        event.stopPropagation()
        player.playOrToggle(context)
      }}
    >
      <Icon className="fill-current" />
    </Button>
  )
}
