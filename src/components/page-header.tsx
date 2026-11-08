import { useQuery } from '@tanstack/react-query'
import type { ReactNode } from 'react'

import { queries } from '@/lib/query'
import { glowOf, neutralTint } from '@/lib/tint'

/** Coloured top of a detail page: artwork, kind, title and a meta line, then a fading bar for actions. */
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
  eyebrow: string
  title: string
  cover: ReactNode
  /** Path of the cover image; its dominant colour tints the header, and `tint` is used until it is known or when there is none. */
  art?: string | null
  meta: ReactNode
  /** Colour while the cover colour is unknown or there is no cover; neutral grey unless given. */
  tint?: string
  onTitleClick?: (() => void) | undefined
  children?: ReactNode
}) {
  const { data: color } = useQuery(queries.coverColor(art))
  const glow = color == null ? tint : glowOf(color)
  return (
    <div
      style={{
        background: `linear-gradient(${glow}, oklch(from ${glow} calc(l * 0.55) c h) 280px, var(--surface) 480px)`,
      }}
    >
      <header className="flex items-end gap-5 px-6 pt-10 pb-5">
        {/* the shadow goes on the cover itself, so a round artist picture casts a round one */}
        <div className="size-[clamp(7rem,13vw,10rem)] shrink-0 [&>*]:size-full [&>*]:shadow-2xl [&>*]:shadow-black/50">
          {cover}
        </div>
        <div className="flex min-w-0 flex-col gap-1.5">
          <span className="text-sm font-medium">{eyebrow}</span>
          <h1
            onClick={onTitleClick}
            className="line-clamp-2 pb-1 text-[clamp(1.75rem,4vw,3.5rem)] leading-[1.05] font-black tracking-tight break-words data-[editable=true]:cursor-pointer"
            data-editable={onTitleClick !== undefined}
          >
            {title}
          </h1>
          <div className="flex flex-wrap items-center gap-1 text-sm text-foreground/80">{meta}</div>
        </div>
      </header>
      {children !== undefined && <div className="flex items-center gap-6 bg-black/15 px-6 py-4">{children}</div>}
    </div>
  )
}

/** Dot between parts of a meta line. */
export function MetaDot() {
  return <span aria-hidden>•</span>
}
