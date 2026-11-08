import { LinkIcon } from 'lucide-react'
import type { Service } from '@/bindings'
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
          <rect x="1" y="4.5" width="22" height="15" rx="4.5" fill="#ff0033" />
          <path d="M10 8.75v6.5L15.5 12z" fill="#fff" />
        </svg>
      )
    case 'youtubeMusic':
      return (
        <svg viewBox="0 0 24 24" className={classes} aria-label={serviceNames.youtubeMusic}>
          <circle cx="12" cy="12" r="11" fill="#ff0033" />
          <circle cx="12" cy="12" r="6.25" fill="none" stroke="#fff" strokeWidth="1.25" />
          <path d="M10.25 9.25v5.5L15 12z" fill="#fff" />
        </svg>
      )
    case 'spotify':
      return (
        <svg viewBox="0 0 24 24" className={classes} aria-label={serviceNames.spotify}>
          <circle cx="12" cy="12" r="11" fill="#1ed760" />
          <g fill="none" stroke="#000" strokeLinecap="round">
            <path d="M6.5 9.25c3.75-1.1 7.75-.8 11 1" strokeWidth="1.9" />
            <path d="M7 12.4c3.1-.85 6.3-.55 9 .85" strokeWidth="1.6" />
            <path d="M7.5 15.3c2.5-.6 4.9-.4 7 .65" strokeWidth="1.3" />
          </g>
        </svg>
      )
    case null:
      return <LinkIcon className={cn(classes, 'text-muted-foreground')} />
    default:
      return service satisfies never
  }
}
