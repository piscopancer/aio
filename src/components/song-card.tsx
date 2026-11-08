import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

/** A year tag as typed: empty, or four digits. */
export const YEAR_PATTERN = /^\d{4}$/

/** Keeps only the digits of typed text, for the year and number fields. */
export const digitsOf = (text: string) => text.replace(/\D/g, '')

/** The bordered card a song's tags are edited in: artwork and fields, a strip of facts below. */
export function SongCard({ children, facts }: { children: ReactNode; facts: ReactNode }) {
  return (
    <div className="overflow-hidden rounded-xl border bg-background/40">
      <div className="flex gap-5 p-4">{children}</div>
      <dl className="flex gap-8 border-t px-4 py-2.5 text-sm">{facts}</dl>
    </div>
  )
}

/** One read-only fact of the song, with a small caption over it. */
export function Fact({ label, className, children }: { label: string; className?: string; children: ReactNode }) {
  return (
    <div className={cn('flex min-w-0 flex-col gap-0.5', className)}>
      <dt className="text-[0.625rem] font-semibold tracking-wider text-subtle-foreground uppercase">{label}</dt>
      <dd className="flex items-center gap-1.5 text-muted-foreground tabular-nums">{children}</dd>
    </div>
  )
}
