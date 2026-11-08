import { useMutation, useQuery } from '@tanstack/react-query'
import { revealItemInDir } from '@tauri-apps/plugin-opener'
import { CircleAlertIcon, FolderOpenIcon, InfoIcon, LoaderCircleIcon, LockIcon } from 'lucide-react'
import { type ReactNode, useState } from 'react'
import { type CoverChange, TRACK_NUMBER_MAX, type Track, type TrackTags, UNKNOWN_ARTIST } from '@/bindings'
import { Cover } from '@/components/cover'
import { CoverPicker } from '@/components/cover-picker'
import { Hint } from '@/components/hint'
import { AlbumField, ArtistsField, findAlbum, leading } from '@/components/library-fields'
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
import { TextField } from '@/components/ui/text-field'
import { formatClock, plural } from '@/lib/format'
import { mutations, queries } from '@/lib/query'
import { closeTrackEditor, useTrackEditor } from '@/lib/track-editor'

/** Dialog that edits the tags of the song `editTrack` was last given; mounted once in the shell. */
export function TrackEditDialog() {
  const track = useTrackEditor((state) => state.track)
  return (
    <Dialog open={track !== null} onOpenChange={(open) => !open && closeTrackEditor()}>
      {track !== null && (
        <DialogContent className="max-w-xl" aria-describedby={undefined}>
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
  const [album, setAlbum] = useState(tags.album ?? '')
  const [year, setYear] = useState(tags.year?.toString() ?? '')
  const [trackNo, setTrackNo] = useState(tags.trackNo?.toString() ?? '')
  const [discNo, setDiscNo] = useState(tags.discNo?.toString() ?? '')
  const [genre, setGenre] = useState(tags.genre ?? '')
  const [cover, setCover] = useState<CoverChange>({ kind: 'keep' })
  const { data: albums = [] } = useQuery(queries.albums())
  const save = useMutation(mutations.editTrack())

  // the same pair the library files an album under: its title, and the album artist or else the main artist
  const owner = tags.albumArtist ?? artists[0] ?? UNKNOWN_ARTIST
  const target = findAlbum(albums, album, owner)
  const joined = target !== undefined && target.id !== track.albumId ? target : null
  const own = target?.id === track.albumId ? target : null
  const initialYear = tags.year?.toString() ?? ''
  const artChanged = cover.kind !== 'keep' || year !== initialYear
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
              album: album.trim() === '' ? null : album,
              year: joined === null ? numberOrNull(year) : joined.year,
              trackNo: numberOrNull(trackNo),
              discNo: numberOrNull(discNo),
              genre: genre.trim() === '' ? null : genre,
              cover: joined === null ? cover : { kind: 'keep' },
            },
          },
          { onSuccess: closeTrackEditor },
        )
      }}
    >
      <DialogBody>
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
          {joined === null ? (
            <CoverPicker
              removable={coverNow !== null}
              onPick={(path) => setCover({ kind: 'set', path })}
              onRemove={() => setCover({ kind: 'remove' })}
              className="size-40"
            >
              <Cover src={coverNow} kind="album" className="size-full shadow-lg" />
            </CoverPicker>
          ) : (
            <div className="relative size-40 shrink-0">
              <Cover src={coverNow} kind="album" className="size-full shadow-lg" />
              <LockIcon className="absolute right-2 bottom-2 size-5 rounded-full bg-black/70 p-1" />
            </div>
          )}
          <div className="grid min-w-0 flex-1 grid-cols-[1fr_4.5rem_4.5rem] content-start gap-3">
            <TextField
              label="Title"
              placeholder={tags.title}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              onClear={() => setTitle('')}
              className="col-span-3"
            />
            <ArtistsField
              placeholder={tags.artists.join(', ')}
              value={artists}
              onValueChange={setArtists}
              className="col-span-3"
            />
            <AlbumField
              placeholder={tags.album ?? undefined}
              value={album}
              onValueChange={setAlbum}
              owner={owner}
              onPick={(picked) => tags.albumArtist === null && setArtists(leading(picked.artist, artists))}
              className="col-span-2"
            />
            <TextField
              label="Year"
              placeholder={tags.year?.toString()}
              inputMode="numeric"
              maxLength={4}
              value={joined === null ? year : (joined.year?.toString() ?? '')}
              disabled={joined !== null}
              aria-invalid={!yearValid}
              onChange={(e) => setYear(digitsOf(e.target.value))}
              className="[&_input]:tabular-nums"
            />
            <TextField
              label="Genre"
              placeholder={tags.genre ?? undefined}
              value={genre}
              onChange={(e) => setGenre(e.target.value)}
              onClear={() => setGenre('')}
            />
            <TextField
              label="Track"
              placeholder={tags.trackNo?.toString()}
              inputMode="numeric"
              maxLength={3}
              value={trackNo}
              aria-invalid={!numberValid(trackNo)}
              onChange={(e) => setTrackNo(digitsOf(e.target.value))}
              className="[&_input]:tabular-nums"
            />
            <TextField
              label="Disc"
              placeholder={tags.discNo?.toString()}
              inputMode="numeric"
              maxLength={3}
              value={discNo}
              aria-invalid={!numberValid(discNo)}
              onChange={(e) => setDiscNo(digitsOf(e.target.value))}
              className="[&_input]:tabular-nums"
            />
          </div>
        </SongCard>
        {own !== null && others > 0 && artChanged && (
          <Note icon={<InfoIcon />}>
            The new cover and year also go to the other {plural(others, 'song')} of {own.title}
          </Note>
        )}
      </DialogBody>
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

function Note({ icon, children }: { icon: ReactNode; children: ReactNode }) {
  return (
    <p className="flex items-center gap-2 text-sm text-muted-foreground [&_svg]:size-4 [&_svg]:shrink-0">
      {icon}
      <span>{children}</span>
    </p>
  )
}
