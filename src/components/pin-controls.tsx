import { PinIcon, PinOffIcon } from 'lucide-react'
import type { PinTarget } from '@/bindings'
import { Hint } from '@/components/hint'
import { Button } from '@/components/ui/button'
import { ContextMenuItem } from '@/components/ui/context-menu'
import { usePinToggle } from '@/lib/pins'
import { cn } from '@/lib/utils'

/** Small accent pin that marks a pinned item. */
export function PinMarker({ className }: { className?: string }) {
  return <PinIcon aria-label="Pinned" className={cn('size-3.5 shrink-0 rotate-45 fill-current text-primary', className)} />
}

/** Badge sizes: on a grid card cover, and on the small cover of a sidebar row. */
const PIN_BADGE_SIZES = {
  card: { badge: 'top-1.5 right-1.5 size-6', icon: 'size-3.5' },
  row: { badge: '-top-1 -right-1 size-4.5', icon: 'size-2.5' },
} as const

/** Pin in a dark round badge at the top-right corner of a pinned item's cover; the cover's wrapper must be `relative`. */
export function PinBadge({ size }: { size: keyof typeof PIN_BADGE_SIZES }) {
  const classes = PIN_BADGE_SIZES[size]
  return (
    <span
      className={cn(
        'pointer-events-none absolute flex items-center justify-center rounded-full bg-black/70 shadow-md',
        classes.badge,
      )}
    >
      <PinMarker className={classes.icon} />
    </span>
  )
}

/** Context menu entry that pins or unpins `target`. */
export function PinMenuItem({ target }: { target: PinTarget }) {
  const { pinned, toggle } = usePinToggle(target)
  const Icon = pinned ? PinOffIcon : PinIcon
  return (
    <ContextMenuItem onSelect={toggle}>
      <Icon />
      {pinned ? 'Unpin' : 'Pin'}
    </ContextMenuItem>
  )
}

/** Button beside a page's Play button that pins or unpins what the page shows. */
export function PinButton({ target }: { target: PinTarget }) {
  const { pinned, toggle } = usePinToggle(target)
  return (
    <Hint label={pinned ? 'Unpin' : 'Pin'}>
      <Button
        variant="ghost"
        size="icon"
        aria-pressed={pinned}
        onClick={toggle}
        className={cn('size-10', pinned ? 'text-primary hover:text-primary' : 'text-muted-foreground')}
      >
        <PinIcon className={cn('size-6', pinned && 'fill-current')} />
      </Button>
    </Hint>
  )
}
