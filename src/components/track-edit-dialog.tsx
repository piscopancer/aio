import { useMutation, useQuery } from '@tanstack/react-query'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import {
  CalendarDaysIcon,
  CircleAlertIcon,
  Disc3Icon,
  DiscIcon,
  FolderOpenIcon,
  GlobeIcon,
  HashIcon,
  InfoIcon,
  LoaderCircleIcon,
  LockIcon,
  MessageSquareTextIcon,
  PenLineIcon,
  PianoIcon,
  ShapesIcon,
  TypeIcon,
  UsersIcon,
} from 'lucide-react'
import { type ReactNode, useState } from 'react'
import {
  type CoverChange,
  type ReleaseType,
  TRACK_NUMBER_MAX,
  type TagMatch,
  type Track,
  type TrackTags,
} from '@/bindings'
import { Cover } from '@/components/cover'
import { CoverPicker } from '@/components/cover-picker'
import { CoverVariants, FoundPanel, Variants, variantsOf } from '@/components/found-tags'
import { Hint } from '@/components/hint'
import {
  AlbumField,
  ArtistsField,
  GenresField,
  findAlbum,
  leading,
  ownAlbumArtists,
  ownedBy,
  sameNames,
} from '@/components/library-fields'
import { Fact, SongCard, YEAR_PATTERN, digitsOf } from '@/components/song-card'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { FieldLabel } from '@/components/ui/field-label'
import { TextAreaField } from '@/components/ui/text-area-field'
import { TextField } from '@/components/ui/text-field'
import { RELEASE_TYPE_LABELS, formatClock, plural } from '@/lib/format'
import { mutations, queries } from '@/lib/query'
import { closeTrackEditor, useTrackEditor } from '@/lib/track-editor'
import { cn } from '@/lib/utils'

/** Dialog that edits the tags of the song `editTrack` was last given; mounted once in the shell. */
export function TrackEditDialog() {
  const track = useTrackEditor((state) => state.track)
  return (
    <Dialog open={track !== null} onOpenChange={(open) => !open && closeTrackEditor()}>
      {track !== null && (
        <DialogContent className="max-w-5xl has-data-found:max-w-7xl" aria-describedby={undefined}>
          <DialogHeader>
            <DialogTitle>Edit details</DialogTitle>
          </DialogHeader>
          <TagsLoader key={track.id} track={track} />
        </DialogContent>
      )}
    </Dialog>
  )
}

function TagsLoader({ track }: { track: Track }) {
  const tags = useQuery(queries.trackTags(track.id))
  if (tags.data !== undefined) return <TagsForm track={track} tags={tags.data} />
  return (
    <DialogBody>
      <p className="flex items-center gap-2 text-sm text-muted-foreground [&_svg]:size-4">
        {tags.error === null ? (
          <LoaderCircleIcon className="animate-spin" />
        ) : (
          <CircleAlertIcon className="text-destructive" />
        )}
        {tags.error?.message ?? 'Reading the file…'}
      </p>
    </DialogBody>
  )
}

/** A track or disc number as typed: empty, or a number in range. */
const numberValid = (text: string) => text === '' || (Number(text) >= 1 && Number(text) <= TRACK_NUMBER_MAX)

const numberOrNull = (text: string) => (text === '' ? null : Number(text))

/** The cover a change leads to, given the album's cover now. */
function shownCover(change: CoverChange, current: string | null) {
  switch (change.kind) {
    case 'keep':
      return current
    case 'set':
      return change.path
    case 'remove':
      return null
    default:
      return change satisfies never
  }
}

function TagsForm({ track, tags }: { track: Track; tags: TrackTags }) {
  const [title, setTitle] = useState(tags.title)
  const [artists, setArtists] = useState(tags.artists)
  // a file without album artists belongs to its main artist, so the field shows who that is
  const [albumArtists, setAlbumArtists] = useState(
    tags.albumArtists.length > 0 ? tags.albumArtists : tags.artists.slice(0, 1),
  )
  const [album, setAlbum] = useState(tags.album ?? '')
  const [year, setYear] = useState(tags.year?.toString() ?? '')
  const [releaseType, setReleaseType] = useState(tags.releaseType)
  const [trackNo, setTrackNo] = useState(tags.trackNo?.toString() ?? '')
  const [discNo, setDiscNo] = useState(tags.discNo?.toString() ?? '')
  const [genres, setGenres] = useState(tags.genres)
  const [composers, setComposers] = useState(tags.composers)
  const [lyricists, setLyricists] = useState(tags.lyricists)
  const [comment, setComment] = useState(tags.comment ?? '')
  const [cover, setCover] = useState<CoverChange>({ kind: 'keep' })
  const { data: albums = [] } = useQuery(queries.albums())
  const save = useMutation(mutations.editTrack())
  const search = useMutation(mutations.searchTags())
  const matches = search.data?.matches ?? []
  const searched = search.data !== undefined || search.isPending
  const setFoundCover = (path: string) => setCover({ kind: 'set', path })
  const listed = (genre: string) => genres.some((known) => known.toLowerCase() === genre.toLowerCase())
  /** Adds a found genre after the ones the song already names. */
  const addGenre = (genre: string) => !listed(genre) && setGenres([...genres, genre])
  /** Takes every tag a find knows; the ones it does not know stay as they are. */
  const apply = (match: TagMatch) => {
    setTitle(match.title)
    if (match.artists.length > 0) setArtists(match.artists)
    if (match.album !== null) setAlbum(match.album)
    if (match.year !== null) setYear(match.year.toString())
    if (match.trackNo !== null) setTrackNo(match.trackNo.toString())
    if (match.discNo !== null) setDiscNo(match.discNo.toString())
    if (match.genre !== null) addGenre(match.genre)
  }
  /** The found values of a field other than the one it holds. */
  const found = (field: (match: TagMatch) => string | number | null, current: string) =>
    variantsOf(
      matches.map((match) => field(match)?.toString() ?? null),
      current,
    )

  const main = artists[0] ?? null
  const ownOwners = ownAlbumArtists(albumArtists, main)
  // the same pair the library files an album under: its title, and the album artists or else the main artist
  const owners = ownOwners.length > 0 ? ownOwners : artists.slice(0, 1)
  const current = albums.find((item) => item.id === track.albumId)
  // new album artists under the same title hand the whole album over, as the backend does, instead of moving the song alone
  const rehomed =
    !sameNames(ownOwners, ownAlbumArtists(tags.albumArtists, tags.artists[0] ?? null)) &&
    current !== undefined &&
    current.title === album.trim() &&
    !ownedBy(current, owners)
  const target = rehomed ? current : findAlbum(albums, album, owners)
  const joined = target !== undefined && target.id !== track.albumId ? target : null
  const own = target?.id === track.albumId ? target : null
  // what of the album this edit changes, which goes to its other songs as well
  const shared = [
    rehomed && 'album artists',
    cover.kind !== 'keep' && 'cover',
    year !== (tags.year?.toString() ?? '') && 'year',
    releaseType !== tags.releaseType && 'release type',
  ].filter((part) => part !== false)
  const others = own === null ? 0 : own.trackCount - 1

  const yearValid = year === '' || YEAR_PATTERN.test(year)
  const blocked = !yearValid || !numberValid(trackNo) || !numberValid(discNo)
  const coverNow = joined === null ? shownCover(cover, track.cover) : joined.cover

  return (
    <form
      className="flex min-h-0 flex-col"
      onSubmit={(event) => {
        event.preventDefault()
        save.mutate(
          {
            id: track.id,
            edit: {
              title,
              artists,
              albumArtists: [...ownOwners],
              album: album.trim() === '' ? null : album,
              year: joined === null ? numberOrNull(year) : joined.year,
              releaseType: joined === null ? releaseType : joined.releaseType,
              trackNo: numberOrNull(trackNo),
              discNo: numberOrNull(discNo),
              genres,
              composers,
              lyricists,
              comment: comment.trim() === '' ? null : comment,
              cover: joined === null ? cover : { kind: 'keep' },
            },
          },
          { onSuccess: closeTrackEditor },
        )
      }}
    >
      <div className="flex min-h-0 flex-1">
        <DialogBody className="min-w-0 flex-1">
          <div className="flex items-center gap-3 rounded-xl border py-1.5 pr-1.5 pl-4">
            <Hint label={tags.path}>
              <span className="min-w-0 flex-1 truncate text-sm text-muted-foreground select-text">{tags.path}</span>
            </Hint>
            <Button type="button" variant="ghost" size="sm" onClick={() => void revealItemInDir(tags.path)}>
              <FolderOpenIcon />
              Show in folder
            </Button>
          </div>
          <SongCard
            facts={
              <>
                <Fact label="Format">{tags.format}</Fact>
                {tags.bitrateKbps !== null && <Fact label="Bitrate">{tags.bitrateKbps} kbps</Fact>}
                <Fact label="Length">{formatClock(tags.durationMs)}</Fact>
              </>
            }
          >
            {/* the song and its album side by side while the card is wide enough for both, one above the other once it is not */}
            <div className="@container min-w-0 flex-1">
              <div className="flex flex-col gap-4 @min-[52rem]:flex-row @min-[52rem]:items-start">
                <div className="grid min-w-0 flex-1 grid-cols-2 content-start gap-3">
                  <Field className="col-span-2" variants={found((match) => match.title, title)} onPick={setTitle}>
                    <TextField
                      label="Title"
                      icon={TypeIcon}
                      hint={HINTS.title}
                      placeholder={tags.title}
                      value={title}
                      onChange={(e) => setTitle(e.target.value)}
                      onClear={() => setTitle('')}
                    />
                  </Field>
                  <Field
                    className="col-span-2"
                    variants={found((match) => match.artists.join(', ') || null, artists.join(', '))}
                    onPick={(credit) =>
                      setArtists(matches.find((match) => match.artists.join(', ') === credit)?.artists ?? artists)
                    }
                  >
                    <ArtistsField
                      hint={HINTS.artists}
                      placeholder={tags.artists.join(', ')}
                      value={artists}
                      onValueChange={setArtists}
                    />
                  </Field>
                  <Field
                    className="col-span-2"
                    variants={found((match) => match.genre, '').filter((genre) => !listed(genre))}
                    onPick={addGenre}
                  >
                    <GenresField hint={HINTS.genres} value={genres} onValueChange={setGenres} />
                  </Field>
                  <div className="col-span-2 grid grid-cols-2 gap-3">
                    <ArtistsField
                      label="Composers"
                      icon={PianoIcon}
                      hint={HINTS.composers}
                      value={composers}
                      onValueChange={setComposers}
                    />
                    <ArtistsField
                      label="Lyricists"
                      icon={PenLineIcon}
                      hint={HINTS.lyricists}
                      value={lyricists}
                      onValueChange={setLyricists}
                    />
                  </div>
                  <TextAreaField
                    label="Comment"
                    icon={MessageSquareTextIcon}
                    hint={HINTS.comment}
                    value={comment}
                    onChange={(e) => setComment(e.target.value)}
                    className="col-span-2"
                  />
                </div>
                <div className="flex min-w-0 flex-1 flex-col gap-4">
                  <section aria-labelledby="album-card" className="flex flex-col gap-3 rounded-lg border p-3">
                    <h3 id="album-card" className="flex items-center gap-1.5 text-sm font-semibold">
                      <Disc3Icon className="size-4 text-muted-foreground" />
                      Album
                    </h3>
                    <div className="flex gap-4">
                      {joined === null ? (
                        <div className="flex w-36 shrink-0 flex-col gap-2">
                          <CoverPicker
                            removable={coverNow !== null}
                            onPick={setFoundCover}
                            onRemove={() => setCover({ kind: 'remove' })}
                            className="size-36"
                          >
                            <Cover src={coverNow} kind="album" className="size-full shadow-lg" />
                          </CoverPicker>
                          <CoverVariants matches={matches} onPick={setFoundCover} />
                        </div>
                      ) : (
                        <div className="relative size-36 shrink-0">
                          <Cover src={coverNow} kind="album" className="size-full shadow-lg" />
                          <LockIcon className="absolute right-2 bottom-2 size-5 rounded-full bg-black/70 p-1" />
                        </div>
                      )}
                      <div className="grid min-w-0 flex-1 grid-cols-[4.5rem_1fr] content-start gap-3">
                        <Field className="col-span-2" variants={found((match) => match.album, album)} onPick={setAlbum}>
                          <AlbumField
                            label="Title"
                            icon={TypeIcon}
                            hint={HINTS.albumTitle}
                            placeholder={tags.album ?? undefined}
                            value={album}
                            onValueChange={setAlbum}
                            owners={owners}
                            onPick={(picked) => {
                              const names = picked.artists.map((artist) => artist.name)
                              setAlbumArtists(names)
                              // a song filed under its main artist follows the picked album's one artist as its lead
                              if (ownOwners.length === 0 && names.length <= 1)
                                setArtists(leading(names[0] ?? null, artists))
                            }}
                          />
                        </Field>
                        <ArtistsField
                          icon={UsersIcon}
                          hint={HINTS.albumArtists}
                          placeholder={main ?? undefined}
                          value={albumArtists}
                          onValueChange={setAlbumArtists}
                          className="col-span-2"
                        />
                        <Field variants={joined === null ? found((match) => match.year, year) : []} onPick={setYear}>
                          <TextField
                            label="Year"
                            icon={CalendarDaysIcon}
                            hint={HINTS.year}
                            placeholder={tags.year?.toString()}
                            inputMode="numeric"
                            maxLength={4}
                            value={joined === null ? year : (joined.year?.toString() ?? '')}
                            disabled={joined !== null}
                            aria-invalid={!yearValid}
                            onChange={(e) => setYear(digitsOf(e.target.value))}
                            className="[&_input]:tabular-nums"
                          />
                        </Field>
                        <ReleaseTypeField
                          value={joined === null ? releaseType : joined.releaseType}
                          onValueChange={setReleaseType}
                          disabled={joined !== null}
                        />
                      </div>
                    </div>
                  </section>
                  <div className="grid grid-cols-2 gap-3">
                    <Field variants={found((match) => match.discNo, discNo)} onPick={setDiscNo}>
                      <TextField
                        label="Disc"
                        icon={DiscIcon}
                        hint={HINTS.disc}
                        placeholder={tags.discNo?.toString()}
                        inputMode="numeric"
                        maxLength={3}
                        value={discNo}
                        aria-invalid={!numberValid(discNo)}
                        onChange={(e) => setDiscNo(digitsOf(e.target.value))}
                        className="[&_input]:tabular-nums"
                      />
                    </Field>
                    <Field variants={found((match) => match.trackNo, trackNo)} onPick={setTrackNo}>
                      <TextField
                        label="Track"
                        icon={HashIcon}
                        hint={HINTS.track}
                        placeholder={tags.trackNo?.toString()}
                        inputMode="numeric"
                        maxLength={3}
                        value={trackNo}
                        aria-invalid={!numberValid(trackNo)}
                        onChange={(e) => setTrackNo(digitsOf(e.target.value))}
                        className="[&_input]:tabular-nums"
                      />
                    </Field>
                  </div>
                </div>
              </div>
            </div>
          </SongCard>
          <Button
            type="button"
            variant="outline"
            disabled={search.isPending}
            onClick={() => search.mutate({ id: track.id, query: { title, artists } })}
          >
            {search.isPending ? <LoaderCircleIcon className="animate-spin" /> : <GlobeIcon />}
            {search.data === undefined ? 'Find online' : 'Search again'}
          </Button>
          {own !== null && others > 0 && shared.length > 0 && (
            <Note icon={<InfoIcon />}>
              The new {CHANGES.format(shared)} also go to the other {plural(others, 'song')} of {own.title}
            </Note>
          )}
        </DialogBody>
        {searched && (
          <FoundPanel search={search.data} pending={search.isPending} onApply={apply} onCover={setFoundCover} />
        )}
      </div>
      <DialogFooter>
        <DialogClose asChild>
          <Button type="button" variant="ghost">
            Cancel
          </Button>
        </DialogClose>
        <Button type="submit" variant="light" disabled={blocked || save.isPending}>
          {save.isPending && <LoaderCircleIcon className="animate-spin" />}
          Save
        </Button>
      </DialogFooter>
    </form>
  )
}

/** Joins the parts of an album an edit changes into one phrase, such as `cover and year`. */
const CHANGES = new Intl.ListFormat('en', { type: 'conjunction' })

/** What each field of the form stands for in the file, shown on hover over its title; tag names go in backticks. */
const HINTS = {
  title: 'The name of the song, kept in the title tag (`TIT2`).',
  artists:
    'Who performs the song, the first one leading. The artist tag (`TPE1`) holds them as one line, and the `ARTISTS` tag lists them one by one.',
  genres: 'The styles of the song, several at once if needed, all kept in the genre tag (`TCON`).',
  composers:
    'Who wrote the music, kept in the composer tag (`TCOM`). Only the file names them; they do not become library artists.',
  lyricists:
    'Who wrote the words, kept in the lyricist tag (`TEXT`). Only the file names them; they do not become library artists.',
  comment: 'Free notes about the song, kept in the comment tag (`COMM`). Players show it but do nothing with it.',
  disc: 'Which disc of a release with several discs the song is on, kept in the disc tag (`TPOS`).',
  track: 'The place of the song on its disc, kept in the track tag (`TRCK`). The album page orders its songs by it.',
  albumTitle:
    'The name of the album, kept in the album tag (`TALB`). Songs with the same album title and album artists make up one album.',
  albumArtists:
    'Whose album this is, kept in the album artist tag (`TPE2`). Players group albums by it, so every song of the album carries the same artists.',
  year: 'When the album came out, kept in the date tag (`TDRC`). Every song of the album carries the same year.',
  releaseType:
    'Whether the release is an album, a single or an EP, kept in the `MusicBrainz Album Type` tag. The artist page puts singles and EPs in their own section.',
} as const

/** The release types the field offers, in this order. */
const RELEASE_TYPES = Object.keys(RELEASE_TYPE_LABELS) as ReleaseType[] // keys of a literal record keyed by the union

/** What kind of release the song's album is, as a row of toggles; pressing the chosen one again leaves the type unsaid. */
function ReleaseTypeField({
  value,
  onValueChange,
  disabled,
}: {
  value: ReleaseType | null
  onValueChange: (value: ReleaseType | null) => void
  disabled: boolean
}) {
  return (
    <div role="group" aria-labelledby="release-type" className="flex min-w-0 flex-col gap-1.5">
      <FieldLabel id="release-type" icon={ShapesIcon} hint={HINTS.releaseType}>
        Type
      </FieldLabel>
      <div className="flex h-10 gap-1 rounded-md bg-secondary p-1">
        {RELEASE_TYPES.map((type) => (
          <button
            key={type}
            type="button"
            aria-pressed={value === type}
            disabled={disabled}
            onClick={() => onValueChange(value === type ? null : type)}
            className="flex-1 cursor-pointer rounded text-xs font-semibold text-muted-foreground transition-colors hover:text-foreground disabled:pointer-events-none disabled:opacity-50 aria-pressed:bg-foreground/15 aria-pressed:text-foreground"
          >
            {RELEASE_TYPE_LABELS[type]}
          </button>
        ))}
      </div>
    </div>
  )
}

/** A field of the form with the values the online search found for it under it. */
function Field({
  variants,
  onPick,
  className,
  children,
}: {
  variants: readonly string[]
  onPick: (value: string) => void
  className?: string
  children: ReactNode
}) {
  return (
    <div className={cn('flex min-w-0 flex-col gap-1.5', className)}>
      {children}
      <Variants values={variants} onPick={onPick} />
    </div>
  )
}

function Note({ icon, children }: { icon: ReactNode; children: ReactNode }) {
  return (
    <p className="flex items-center gap-2 text-sm text-muted-foreground [&_svg]:size-4 [&_svg]:shrink-0">
      {icon}
      <span>{children}</span>
    </p>
  )
}
