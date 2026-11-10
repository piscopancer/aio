import { CalendarDaysIcon, ClockIcon, Disc3Icon, type LucideIcon, Music2Icon } from 'lucide-react'
import type { ReactNode } from 'react'

import { CoverGradient } from '@/components/cover-gradient'
import { Halftone } from '@/components/halftone'
import { Hint } from '@/components/hint'
import { formatLength, plural } from '@/lib/format'
import { neutralTint } from '@/lib/tint'

/** Coloured top of a detail page, printed in halftone dots over the flowing colours of its cover, or over a gradient of `tint` without one: artwork, kind, title and a meta line, then a fading bar for actions. */
export function PageHeader({
  eyebrow,
  title,
  cover,
  art = null,
  meta,
  tint = neutralTint,
  onTitleClick,
  children,
}: {
  /** Small line above the title: the kind of page, or what places it, such as where a folder is. */
  eyebrow: ReactNode
  title: string
  cover: ReactNode
  /** Path of the cover image, whose palette flows behind the header as it does behind the lyrics. */
  art?: string | null
  meta: ReactNode
  /** Colour of the gradient a header without a cover shows; neutral grey unless given. */
  tint?: string
  onTitleClick?: (() => void) | undefined
  children?: ReactNode
}) {
  return (
    <div
      className="relative isolate"
      style={
        art === null
          ? {
              background: `linear-gradient(${tint}, oklch(from ${tint} calc(l * 0.55) c h) 280px, var(--surface) 480px)`,
            }
          : undefined
      }
    >
      {art !== null && <CoverGradient cover={art} />}
      <Halftone />
      <header className="flex items-end gap-5 px-6 pt-10 pb-5">
        {/* the shadow goes on the cover itself, so a round artist picture casts a round one */}
        <div className="size-[clamp(7rem,13vw,10rem)] shrink-0 [&>*]:size-full [&>*]:shadow-2xl [&>*]:shadow-black/50">
          {cover}
        </div>
        <div className="flex min-w-0 flex-col gap-1.5">
          <span className="flex min-w-0 text-sm font-medium">{eyebrow}</span>
          <h1
            onClick={onTitleClick}
            className="line-clamp-2 pb-1 text-[clamp(1.75rem,4vw,3.5rem)] leading-[1.05] font-black tracking-tight break-words data-[editable=true]:cursor-pointer"
            data-editable={onTitleClick !== undefined}
          >
            {title}
          </h1>
          <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-foreground/80">{meta}</div>
        </div>
      </header>
      {children !== undefined && <div className="flex items-center gap-6 bg-surface/30 px-6 py-4">{children}</div>}
    </div>
  )
}

/** What each figure of a meta line shows, and the words its tooltip spells it out in. */
const META_STATS = {
  albums: {
    icon: Disc3Icon,
    value: (count: number) => count.toLocaleString(),
    hint: (count: number) => plural(count, 'album'),
  },
  songs: {
    icon: Music2Icon,
    value: (count: number) => count.toLocaleString(),
    hint: (count: number) => plural(count, 'song'),
  },
  length: { icon: ClockIcon, value: formatLength, hint: () => 'Total length' },
  year: { icon: CalendarDaysIcon, value: String, hint: () => 'Release year' },
} as const satisfies Record<string, { icon: LucideIcon; value: (n: number) => string; hint: (n: number) => string }>

/** Dark translucent capsule holding the figures of a meta line, readable over any cover colour. */
export function MetaStats({ children }: { children: ReactNode }) {
  return <span className="flex shrink-0 items-center gap-3 rounded-full bg-black/40 px-3 py-1">{children}</span>
}

/** One figure of a meta line: an icon and the number, named in full by its tooltip. */
export function MetaStat({ kind, value }: { kind: keyof typeof META_STATS; value: number }) {
  const { icon: Icon, value: show, hint } = META_STATS[kind]
  const label = hint(value)
  return (
    <Hint label={label}>
      <span aria-label={label} className="flex shrink-0 cursor-default items-center gap-1.5 tabular-nums">
        <Icon className="size-4 shrink-0" aria-hidden />
        {show(value)}
      </span>
    </Hint>
  )
}
