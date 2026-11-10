import { LinkIcon } from 'lucide-react'
import type { Service } from '@/bindings'
import { Hint } from '@/components/hint'
import { cn } from '@/lib/utils'

/** Hosts each service's song links live on; the backend still decides whether a link is usable. */
const serviceHosts = {
  youtube: ['youtube.com', 'm.youtube.com', 'youtu.be'],
  youtubeMusic: ['music.youtube.com'],
  spotify: ['open.spotify.com', 'play.spotify.com'],
} as const satisfies Record<Service, readonly string[]>

const services = ['youtube', 'youtubeMusic', 'spotify'] as const satisfies readonly Service[]

export const serviceNames = {
  youtube: 'YouTube',
  youtubeMusic: 'YouTube Music',
  spotify: 'Spotify',
} as const satisfies Record<Service, string>

/** Brand colour each service's mark is filled with. */
const serviceColors = {
  youtube: '#ff0033',
  youtubeMusic: '#ff0033',
  spotify: '#1ed760',
} as const satisfies Record<Service, string>

/** The service a pasted link belongs to, judged by its host alone, so the icon shows while the link is still typed. */
export function linkService(text: string) {
  const trimmed = text.trim()
  if (trimmed.startsWith('spotify:')) return 'spotify'
  const host = URL.parse(trimmed.includes('://') ? trimmed : `https://${trimmed}`)?.hostname.replace(/^www\./, '')
  if (host === undefined) return null
  return services.find((service) => serviceHosts[service].some((known) => known === host)) ?? null
}

/** Simplified mark of a streaming service, or a plain link icon when the service is unknown. */
export function ServiceIcon({ service, className }: { service: Service | null; className?: string }) {
  const classes = cn('size-5 shrink-0', className)
  switch (service) {
    case 'youtube':
      return (
        <svg viewBox="0 0 24 24" className={classes} aria-label={serviceNames.youtube}>
          <rect x="1" y="4.5" width="22" height="15" rx="4.5" fill={serviceColors.youtube} />
          <path d="M10 8.75v6.5L15.5 12z" fill="#fff" />
        </svg>
      )
    case 'youtubeMusic':
      return (
        <svg viewBox="0 0 24 24" className={classes} aria-label={serviceNames.youtubeMusic}>
          <circle cx="12" cy="12" r="11" fill={serviceColors.youtubeMusic} />
          <circle cx="12" cy="12" r="6.25" fill="none" stroke="#fff" strokeWidth="1.25" />
          <path d="M10.25 9.25v5.5L15 12z" fill="#fff" />
        </svg>
      )
    case 'spotify':
      return (
        <svg viewBox="0 0 24 24" className={classes} aria-label={serviceNames.spotify}>
          <circle cx="12" cy="12" r="11" fill={serviceColors.spotify} />
          {/* the three tapering arcs of the official mark, drawn for a full 24 unit circle and shrunk to this one */}
          <path
            transform="translate(12 12) scale(0.9167) translate(-12 -12)"
            fill="#000"
            d="M17.521 17.34c-.24.359-.66.48-1.021.24-2.82-1.74-6.36-2.101-10.561-1.141-.418.122-.779-.179-.899-.539-.12-.421.18-.78.54-.9 4.56-1.021 8.52-.6 11.64 1.32.42.18.479.659.301 1.02zM18.961 14.04c-.301.42-.841.6-1.262.3-3.239-1.98-8.159-2.58-11.939-1.38-.479.12-1.02-.12-1.14-.6-.12-.48.12-1.021.6-1.141C9.6 9.9 15 10.561 18.72 12.84c.361.181.54.78.241 1.2zM19.081 10.68C15.24 8.4 8.82 8.16 5.16 9.301c-.6.179-1.2-.181-1.38-.721-.18-.601.18-1.2.72-1.381 4.26-1.26 11.28-1.02 15.721 1.621.539.3.719 1.02.419 1.56-.299.421-1.02.599-1.559.3z"
          />
        </svg>
      )
    case null:
      return <LinkIcon className={cn(classes, 'text-muted-foreground')} />
    default:
      return service satisfies never
  }
}

/** How much each mark is enlarged inside its round badge, so the glyph fills it; Spotify's arcs already do. */
const badgeZoom = {
  youtube: 'scale-[1.35]',
  youtubeMusic: 'scale-[1.15]',
  spotify: '',
} as const satisfies Record<Service, string>

/** The services songs download from, as overlapping round marks like a stack of avatars, each named by its tooltip. */
export function ServiceStack({ className }: { className?: string }) {
  return (
    <div className={cn('flex -space-x-1.5', className)}>
      {services.map((service) => (
        <Hint key={service} label={serviceNames[service]}>
          {/* filled with the brand colour, so a mark narrower than the circle blends into it; isolated, so each enlarged mark paints with its own circle and every circle overlaps the one before it */}
          <span
            className="relative isolate flex size-5 shrink-0 overflow-hidden rounded-full ring-2 ring-card"
            style={{ backgroundColor: serviceColors[service] }}
          >
            <ServiceIcon service={service} className={cn('size-full', badgeZoom[service])} />
          </span>
        </Hint>
      ))}
    </div>
  )
}
