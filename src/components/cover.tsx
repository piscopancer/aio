import { Disc3Icon, FolderIcon, ListMusicIcon, MicVocalIcon, MusicIcon } from 'lucide-react'
import type { CoverSource } from '@/bindings'
import { coverSrc } from '@/lib/format'
import { cn } from '@/lib/utils'

const fallbackIcons = {
  album: Disc3Icon,
  artist: MicVocalIcon,
  playlist: ListMusicIcon,
  track: MusicIcon,
  folder: FolderIcon,
} as const

/** Peach light of a pinned cover, shining from the top-right corner where the pin badge sits towards the opposite one. */
const PINNED_GLOW =
  'pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top_right,oklch(from_var(--primary)_calc(l+0.02)_calc(c*1.4)_h/0.85),transparent_50%)] mix-blend-screen'

/** Artwork of a library item, or an icon of its kind when it has none; a `pinned` one glows from its top-right corner. */
export function Cover({
  src,
  kind,
  pinned = false,
  className,
}: {
  src: string | null
  kind: keyof typeof fallbackIcons
  pinned?: boolean
  className?: string | undefined
}) {
  const Fallback = fallbackIcons[kind]
  return (
    <div
      className={cn(
        'relative flex aspect-square shrink-0 items-center justify-center overflow-hidden rounded-md bg-secondary text-subtle-foreground',
        kind === 'artist' && 'rounded-full',
        className,
      )}
    >
      {src === null ? (
        <Fallback className="size-2/5" strokeWidth={1.5} />
      ) : (
        <img
          src={coverSrc(src)}
          alt=""
          draggable={false}
          loading="lazy"
          decoding="async"
          className="size-full object-cover"
        />
      )}
      {pinned && <span className={PINNED_GLOW} />}
    </div>
  )
}

/** Artwork of a download, from the site or from a picked file, or a track icon when there is none. */
export function SourceCover({ source, className }: { source: CoverSource | null; className?: string | undefined }) {
  if (source === null) return <Cover src={null} kind="track" className={className} />
  return (
    <img
      src={source.kind === 'web' ? source.url : coverSrc(source.path)}
      alt=""
      draggable={false}
      className={cn('aspect-square shrink-0 rounded-md object-cover', className)}
    />
  )
}
