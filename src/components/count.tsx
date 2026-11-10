import { Disc3Icon, type LucideIcon, Music2Icon } from 'lucide-react'
import type { ReactNode } from 'react'
import { plural } from '@/lib/format'
import { cn } from '@/lib/utils'

/** What a count can count: the icon it shows and the noun its label spells the number out with. */
const COUNTED = {
  songs: { icon: Music2Icon, noun: 'song' },
  albums: { icon: Disc3Icon, noun: 'album' },
} as const satisfies Record<string, { icon: LucideIcon; noun: string }>

/** A number after the small icon of what it counts, read out in full by screen readers. */
export function Count({ kind, value }: { kind: keyof typeof COUNTED; value: number }) {
  const { icon: Icon, noun } = COUNTED[kind]
  return (
    <span aria-label={plural(value, noun)} className="flex shrink-0 items-center gap-1 tabular-nums">
      <Icon className="size-3.5 shrink-0" aria-hidden />
      {value.toLocaleString()}
    </span>
  )
}

/** A muted line of counts side by side, under a name. */
export function Counts({ className, children }: { className?: string; children: ReactNode }) {
  return <span className={cn('flex items-center gap-3 text-sm text-muted-foreground', className)}>{children}</span>
}
