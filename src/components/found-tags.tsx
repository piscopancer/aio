import { useMutation } from '@tanstack/react-query'
import { LoaderCircleIcon } from 'lucide-react'
import { useState } from 'react'
import type { FoundCover, TagMatch, TagSearch, TagSource } from '@/bindings'
import { Button } from '@/components/ui/button'
import { mutations } from '@/lib/query'

/** How the databases the tag search asks are named to the user. */
const sourceNames = {
  acoustId: 'AcoustID',
  musicBrainz: 'MusicBrainz',
  itunesRu: 'iTunes RU',
  itunesUs: 'iTunes US',
  deezer: 'Deezer',
  bandcamp: 'Bandcamp',
  youtubeMusic: 'YouTube Music',
} as const satisfies Record<TagSource, string>

/** How many variants a field offers at most, so a busy search does not push the form apart. */
const VARIANTS_MAX = 5

/** How many found covers are offered at most. */
const COVERS_MAX = 8

/** Text compared the way a user reads it: case and surrounding spaces do not make two variants different. */
const comparable = (text: string) => text.trim().toLocaleLowerCase()

/** The distinct values the finds give a field, in the order found, apart from blanks and the value it holds now. */
export function variantsOf(values: readonly (string | null)[], current: string) {
  const seen = new Set([comparable(current)])
  const variants: string[] = []
  for (const value of values) {
    if (value === null || value.trim() === '' || seen.has(comparable(value))) continue
    seen.add(comparable(value))
    variants.push(value)
  }
  return variants.slice(0, VARIANTS_MAX)
}

/** Found values of one field as small buttons under it; pressing one puts it into the field. */
export function Variants({ values, onPick }: { values: readonly string[]; onPick: (value: string) => void }) {
  if (values.length === 0) return null
  return (
    <div className="flex flex-wrap gap-1">
      {values.map((value) => (
        <button
          key={value}
          type="button"
          onClick={() => onPick(value)}
          className="max-w-full cursor-pointer truncate rounded-full bg-foreground/8 px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:bg-foreground/15 hover:text-foreground"
        >
          {value}
        </button>
      ))}
    </div>
  )
}

/** Downloads and stores a found cover, then hands its stored path to `onPick`; the cover being fetched is shown as
 * such. */
function useCoverPick(onPick: (path: string) => void) {
  const { mutate, isPending, variables } = useMutation(mutations.importFoundCover())
  return {
    pick: (cover: FoundCover) => mutate(cover.full, { onSuccess: onPick }),
    picking: (cover: FoundCover) => isPending && variables === cover.full,
  }
}

/** The covers the finds offer, as small pictures under the cover; pressing one makes it the song's cover. Pictures a
 * database names but does not have are left out. */
export function CoverVariants({ matches, onPick }: { matches: readonly TagMatch[]; onPick: (path: string) => void }) {
  const [missing, setMissing] = useState<ReadonlySet<string>>(new Set())
  const { pick, picking } = useCoverPick(onPick)
  const covers = [
    ...new Map(
      matches.flatMap((match) => (match.cover === null ? [] : [[match.cover.thumb, match.cover] as const])),
    ).values(),
  ]
    .filter((cover) => !missing.has(cover.thumb))
    .slice(0, COVERS_MAX)
  if (covers.length === 0) return null
  return (
    <div className="grid grid-cols-4 gap-1.5">
      {covers.map((cover) => (
        <button
          key={cover.thumb}
          type="button"
          onClick={() => pick(cover)}
          className="relative aspect-square cursor-pointer overflow-hidden rounded-sm bg-foreground/10 ring-primary transition-shadow hover:ring-1"
        >
          <img
            src={cover.thumb}
            alt=""
            loading="lazy"
            onError={() => setMissing((known) => new Set(known).add(cover.thumb))}
            className="size-full object-cover"
          />
          {picking(cover) && (
            <span className="absolute inset-0 flex items-center justify-center bg-black/60">
              <LoaderCircleIcon className="size-4 animate-spin" />
            </span>
          )}
        </button>
      ))}
    </div>
  )
}

/** The side panel of the edit dialog listing what the online search found, each find with a button that takes all
 * its tags and its cover at once. */
export function FoundPanel({
  search,
  pending,
  onApply,
  onCover,
}: {
  search: TagSearch | undefined
  pending: boolean
  onApply: (match: TagMatch) => void
  onCover: (path: string) => void
}) {
  const { pick, picking } = useCoverPick(onCover)
  return (
    <aside data-found className="flex w-80 shrink-0 flex-col border-l">
      <h3 className="flex items-center gap-2 px-4 pt-4 pb-2 text-sm font-semibold">
        {pending ? 'Searching…' : `Found ${search?.matches.length ?? 0}`}
        {pending && <LoaderCircleIcon className="size-4 animate-spin" />}
      </h3>
      <div className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto px-2 pb-4">
        {search?.matches.map((match, index) => (
          <div
            // the same tags can come from two databases, so a find is told apart by its place
            key={index}
            className="flex items-center gap-3 rounded-md p-2 hover:bg-surface-hover"
          >
            <div className="relative size-12 shrink-0 overflow-hidden rounded-sm bg-foreground/10">
              {match.cover !== null && (
                <img
                  src={match.cover.thumb}
                  alt=""
                  loading="lazy"
                  onError={(event) => event.currentTarget.remove()}
                  className="size-full object-cover"
                />
              )}
              {match.cover !== null && picking(match.cover) && (
                <span className="absolute inset-0 flex items-center justify-center bg-black/60">
                  <LoaderCircleIcon className="size-4 animate-spin" />
                </span>
              )}
            </div>
            <div className="flex min-w-0 flex-1 flex-col text-sm">
              <span className="truncate font-medium">{match.title}</span>
              <span className="truncate text-muted-foreground">{match.artists.join(', ')}</span>
              <span className="truncate text-xs text-subtle-foreground">
                {[match.album, match.year, sourceNames[match.source]].filter((part) => part != null).join(' · ')}
              </span>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="shrink-0 hover:bg-foreground/10"
              onClick={() => {
                onApply(match)
                if (match.cover !== null) pick(match.cover)
              }}
            >
              Use
            </Button>
          </div>
        ))}
        {search !== undefined && search.matches.length === 0 && (
          <p className="px-2 py-4 text-sm text-muted-foreground">Nothing found for this song</p>
        )}
        {search !== undefined && search.failed.length > 0 && (
          <p className="px-2 pt-2 text-xs text-subtle-foreground">
            Could not reach {search.failed.map((source) => sourceNames[source]).join(', ')}
          </p>
        )}
      </div>
    </aside>
  )
}
