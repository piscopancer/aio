import { cn } from '@/lib/utils'

const BAR_DELAYS = ['0ms', '-300ms', '-600ms', '-150ms'] as const

/** Animated bars marking the track that is playing right now. */
export function Equalizer({ className }: { className?: string | undefined }) {
  return (
    <span aria-label="Playing" className={cn('inline-flex h-3.5 items-end gap-[2px]', className)}>
      {BAR_DELAYS.map((delay) => (
        <span
          key={delay}
          className="h-full w-[3px] equalizer-bar rounded-[1px] bg-primary"
          style={{ animationDelay: delay }}
        />
      ))}
    </span>
  )
}
