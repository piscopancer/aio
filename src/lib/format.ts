import { convertFileSrc } from '@tauri-apps/api/core'

const MS_PER_SECOND = 1000
const SECONDS_PER_MINUTE = 60
const SECONDS_PER_HOUR = 3600

/** Clock form of a track length or position, `3:07` or `1:02:45`. */
export function formatClock(ms: number) {
  const total = Math.floor(ms / MS_PER_SECOND)
  const hours = Math.floor(total / SECONDS_PER_HOUR)
  const minutes = Math.floor((total % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE)
  const seconds = String(total % SECONDS_PER_MINUTE).padStart(2, '0')
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, '0')}:${seconds}` : `${minutes}:${seconds}`
}

/** Spoken form of a collection length, `1 hr 12 min` or `42 min 5 sec`. */
export function formatLength(ms: number) {
  const total = Math.floor(ms / MS_PER_SECOND)
  const hours = Math.floor(total / SECONDS_PER_HOUR)
  const minutes = Math.floor((total % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE)
  return hours > 0 ? `${hours} hr ${minutes} min` : `${minutes} min ${total % SECONDS_PER_MINUTE} sec`
}

/** `1 song`, `12 songs`. */
export function plural(count: number, noun: string) {
  return `${count.toLocaleString()} ${noun}${count === 1 ? '' : 's'}`
}

/** Webview URL of a cover image the backend extracted to disk. */
export function coverSrc(cover: string | null) {
  return cover === null ? undefined : convertFileSrc(cover)
}

/** File size in the unit that keeps it short, e.g. "3.9 MB". */
export function formatBytes(bytes: number) {
  const units = ['B', 'KB', 'MB', 'GB'] as const
  const exponent = Math.min(units.length - 1, Math.floor(Math.log(Math.max(bytes, 1)) / Math.log(1000)))
  const value = bytes / 1000 ** exponent
  return `${value.toFixed(value < 10 && exponent > 0 ? 1 : 0)} ${units[exponent]}`
}

/** The credited artists of a song in one line, the way the file's plain artist tag writes them. */
export function credit(artists: readonly { name: string }[]) {
  return artists.map((artist) => artist.name).join(', ')
}
