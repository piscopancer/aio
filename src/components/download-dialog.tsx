import { keepPreviousData, useMutation, useQuery } from '@tanstack/react-query'
import { open } from '@tauri-apps/plugin-dialog'
import {
  ArrowLeftIcon,
  CheckIcon,
  CircleAlertIcon,
  CircleCheckIcon,
  ClipboardIcon,
  DownloadIcon,
  FolderIcon,
  FolderPlusIcon,
  LoaderCircleIcon,
  PauseIcon,
  PlayIcon,
  SearchIcon,
  SkipBackIcon,
  SkipForwardIcon,
} from 'lucide-react'
import { type ReactNode, useEffect, useState } from 'react'
import type { AudioQuality, CoverSource, DownloadPreview, FolderCheck, FoundSong, PlaylistId } from '@/bindings'
import { ChipLabel } from '@/components/chip-label'
import { Cover, SourceCover } from '@/components/cover'
import { CoverPicker } from '@/components/cover-picker'
import { DownloadProgress, isActive } from '@/components/downloads-button'
import { Hint } from '@/components/hint'
import { Marquee } from '@/components/marquee'
import { ServiceIcon, ServiceStack, linkService, serviceNames } from '@/components/service-icon'
import { PinMarker } from '@/components/pin-controls'
import { AlbumField, ArtistsField, leading } from '@/components/library-fields'
import { Fact, SongCard, YEAR_PATTERN, digitsOf } from '@/components/song-card'
import { ToolInstallProgress } from '@/components/tool-install'
import { ValueSlider, VolumeControl, useDraft } from '@/components/player-bar'
import { Button } from '@/components/ui/button'
import { ClearButton } from '@/components/ui/clear-button'
import {
  Dialog,
  DialogBody,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { RadioCard, RadioGroup } from '@/components/ui/radio-group'
import { TextField } from '@/components/ui/text-field'
import { useDebounced } from '@/lib/debounce'
import { formatBytes, formatClock } from '@/lib/format'
import { errorMessage } from '@/lib/ipc'
import { useOrderedPlaylists } from '@/lib/playlists'
import { type PreviewStatus, pausePreview, playPreview, seekPreview, stopPreview, usePreview } from '@/lib/preview'
import { mutations, queries } from '@/lib/query'
import { cn } from '@/lib/utils'

/** Pause after the last key press before a typed link is looked up or a typed name searched for; a pasted link is looked up at once. */
const LOOKUP_DELAY_MS = 400

/** How often the paste button checks the clipboard for a song link while the dialog waits for one. */
const CLIPBOARD_POLL_MS = 1000

/** Most artists suggested in the empty dialog, the pinned ones first. */
const SUGGESTED_ARTISTS = 12

/** Labels of the qualities the preview offers. */
const qualityLabels = {
  original: 'Original',
  'mp3-320': 'MP3 320',
  'mp3-192': 'MP3 192',
  'mp3-128': 'MP3 128',
} as const satisfies Record<AudioQuality, string>

/** Where the dialog was opened from: the playlist it pre-checks, or the folder it saves into. */
type Origin = { playlistId: PlaylistId } | { folder: string }

/** The folder a download from `origin` saves into, null for the one in the settings. */
const originFolder = (origin: Origin) => ('folder' in origin ? origin.folder : null)

/** The playlists a download from `origin` goes into. */
const originPlaylists = (origin: Origin) => ('playlistId' in origin ? [origin.playlistId] : [])

/** Dialog that downloads one song, found by its name or from a pasted link, opened by the given trigger. */
export function DownloadDialog({ origin, children }: { origin: Origin; children: ReactNode }) {
  const [open, setOpen] = useState(false)
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{children}</DialogTrigger>
      {open && <DownloadForm origin={origin} onDone={() => setOpen(false)} />}
    </Dialog>
  )
}

function DownloadForm({ origin, onDone }: { origin: Origin; onDone: () => void }) {
  const [text, setText] = useState('')
  /** A pasted or typed link, locked in the field while its song is shown. */
  const [target, setTarget] = useState<string | null>(null)
  /** A song picked from the search results, shown until Back returns to them. */
  const [picked, setPicked] = useState<FoundSong | null>(null)
  const settled = useDebounced(text.trim(), LOOKUP_DELAY_MS)
  // a typed link is looked up, and locked, once typing pauses
  useEffect(() => {
    if (linkService(settled) !== null) setTarget(settled)
  }, [settled])
  // closing the dialog ends the preview, and the main player carries on
  useEffect(() => stopPreview, [])
  /** Looks up a pasted link, also in place of the song shown, which it replaces. */
  const lookUp = (link: string) => {
    setText(link)
    setTarget(link)
    setPicked(null)
  }
  const prepare = useQuery(queries.prepareDownloads())
  const shown = target ?? picked?.videoUrl ?? null
  const preview = useQuery({ ...queries.downloadPreview(shown ?? ''), enabled: shown !== null })
  const service = linkService(shown ?? text)
  // anything typed that is not a link is a name to search for, and an emptied field drops the results at once rather than once the pause settles
  const query = shown === null && text.trim() !== '' && linkService(text) === null ? settled : ''
  /** The name last searched for, kept while a picked song is shown, so the dialog's player can step through its results. */
  const searched = linkService(settled) === null ? settled : ''

  return (
    <DialogContent className="h-[calc(100dvh-4rem)] max-w-3xl">
      <DialogHeader>
        <DialogTitle>Download a song</DialogTitle>
        {/* the marks in the empty field say it to the eye, this to screen readers */}
        <DialogDescription className="sr-only">
          Type a song's name to search YouTube Music, or paste a link to a song on YouTube, YouTube Music or Spotify.
        </DialogDescription>
      </DialogHeader>
      <div className="flex shrink-0 items-center gap-2 px-6 py-6">
        <label className="group/field relative flex min-w-0 flex-1 items-center">
          {service === null ? (
            // ringed with the field's own colour, which lightens while it has the focus
            <ServiceStack className="absolute left-3 *:ring-secondary group-focus-within/field:*:ring-accent" />
          ) : (
            <ServiceIcon service={service} className="pointer-events-none absolute left-3" />
          )}
          <Input
            autoFocus
            aria-label="Song name or link"
            placeholder="Song name or link"
            value={text}
            readOnly={shown !== null}
            onChange={(event) => setText(event.target.value)}
            onPaste={(event) => {
              const pasted = event.clipboardData.getData('text').trim()
              if (linkService(pasted) === null) return
              event.preventDefault()
              lookUp(pasted)
            }}
            className={cn(
              'read-only:text-muted-foreground',
              service === null ? 'pl-[4.5rem]' : 'pl-11',
              text !== '' && 'pr-9',
            )}
          />
          {text !== '' && (
            <ClearButton
              onClear={() => {
                setText('')
                setTarget(null)
                setPicked(null)
              }}
            />
          )}
        </label>
        <PasteButton onPaste={lookUp} />
      </div>
      {picked !== null && (
        <div className="-mt-4 shrink-0 px-6 pb-2">
          <Button type="button" variant="ghost" className="px-0" onClick={() => setPicked(null)}>
            <ArrowLeftIcon />
            Back
          </Button>
        </div>
      )}
      {text.trim() === '' && <ArtistSuggestions onPick={setText} />}
      {query !== '' && <SearchResults query={query} origin={origin} onPick={setPicked} />}
      {shown !== null && service !== null && (
        <ToolsSetup failed={prepare.error?.message}>
          {preview.isFetching ? (
            <DialogBody className="pt-0">
              <Status icon={<LoaderCircleIcon className="animate-spin" />}>
                Looking up the song on {serviceNames[service]}…
              </Status>
            </DialogBody>
          ) : preview.error !== null ? (
            <DialogBody className="pt-0">
              <Status icon={<CircleAlertIcon className="text-destructive" />}>{preview.error.message}</Status>
            </DialogBody>
          ) : (
            preview.data !== undefined && (
              <PreviewForm
                key={preview.data.videoUrl}
                preview={preview.data}
                found={picked}
                origin={origin}
                onDone={onDone}
              />
            )
          )}
        </ToolsSetup>
      )}
      {/* outside both views, so the song keeps playing when it is picked and while its form is shown */}
      <PreviewBar query={searched} />
    </DialogContent>
  )
}

/** Pills of the pinned artists, then of those with the most songs, that search for more of their songs while the field is empty. */
function ArtistSuggestions({ onPick }: { onPick: (name: string) => void }) {
  const { data: artists = [] } = useQuery(queries.artists())
  const pinned = artists.filter((artist) => artist.pin !== null).toSorted((a, b) => (a.pin ?? 0) - (b.pin ?? 0))
  const largest = artists.filter((artist) => artist.pin === null).toSorted((a, b) => b.trackCount - a.trackCount)
  const suggested = [...pinned, ...largest].slice(0, SUGGESTED_ARTISTS)
  if (suggested.length === 0) return null
  return (
    <DialogBody className="pt-0">
      <div className="flex flex-wrap gap-2">
        {suggested.map((artist) => (
          <button
            key={artist.id}
            type="button"
            onClick={() => onPick(artist.name)}
            className="flex h-9 cursor-pointer items-center gap-2 rounded-full bg-secondary py-1 pr-3.5 pl-1 text-sm font-medium transition-colors hover:bg-accent"
          >
            <Cover src={artist.cover} kind="artist" className="size-7" />
            <ChipLabel>{artist.name}</ChipLabel>
            {artist.pin !== null && <PinMarker />}
          </button>
        ))}
      </div>
    </DialogBody>
  )
}

/** The songs, then the videos, a typed name found; the earlier results stay, dimmed, while the next ones load. */
function SearchResults({
  query,
  origin,
  onPick,
}: {
  query: string
  origin: Origin
  onPick: (found: FoundSong) => void
}) {
  const search = useQuery({ ...queries.downloadSearch(query), placeholderData: keepPreviousData })
  if (search.error !== null)
    return (
      <DialogBody className="pt-0">
        <Status icon={<CircleAlertIcon className="text-destructive" />}>{search.error.message}</Status>
      </DialogBody>
    )
  if (search.data === undefined)
    return (
      <DialogBody className="pt-0">
        <Status icon={<LoaderCircleIcon className="animate-spin" />}>Searching YouTube Music…</Status>
      </DialogBody>
    )
  const { songs, videos } = search.data
  if (songs.length === 0 && videos.length === 0)
    return (
      <DialogBody className="pt-0">
        <Status icon={<SearchIcon />}>Nothing found for “{query}”</Status>
      </DialogBody>
    )
  return (
    <DialogBody className={cn('pt-0 transition-opacity', search.isPlaceholderData && 'opacity-60')}>
      {/* one box for both shelves, so the playing row stays stuck to an edge while scrolling through either */}
      <div className="-mx-2 flex min-w-0 flex-col gap-0.5">
        {songs.length > 0 && <ShelfTitle>Songs</ShelfTitle>}
        {songs.map((song) => (
          <FoundRow key={song.videoUrl} found={song} origin={origin} onPick={onPick} />
        ))}
        {videos.length > 0 && <ShelfTitle className={cn(songs.length > 0 && 'mt-4')}>Videos</ShelfTitle>}
        {videos.map((video) => (
          <FoundRow key={video.videoUrl} found={video} origin={origin} onPick={onPick} />
        ))}
      </div>
    </DialogBody>
  )
}

function ShelfTitle({ className, children }: { className?: string | false | undefined; children: ReactNode }) {
  return <h3 className={cn('px-2 pb-1.5 text-sm font-semibold text-muted-foreground', className)}>{children}</h3>
}

/** A found song: its cover plays it in the dialog's player, the button at its end downloads it as found, and the rest of the row picks it for the full form; the row playing sticks to the top or bottom edge while scrolled away. */
function FoundRow({ found, origin, onPick }: { found: FoundSong; origin: Origin; onPick: (found: FoundSong) => void }) {
  const status = usePreview((state) => (state.song?.videoUrl === found.videoUrl ? state.status : null))
  return (
    <div
      className={cn(
        'group relative flex min-w-0 items-center gap-3 rounded-md p-2 hover:bg-surface-hover',
        status !== null && 'sticky inset-y-0 z-20 bg-secondary hover:bg-secondary',
      )}
    >
      <div className="relative z-10 shrink-0">
        <FoundCover found={found} className="size-12" />
        <button
          type="button"
          aria-label={status === 'playing' ? 'Pause' : 'Listen'}
          onClick={() => (status === 'playing' ? pausePreview() : void playPreview(found))}
          className={cn(
            'absolute inset-0 flex cursor-pointer items-center justify-center rounded-md bg-black/50 text-white opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100 [&_svg]:size-5',
            status !== null && 'opacity-100',
          )}
        >
          <PreviewIcon status={status ?? 'paused'} />
        </button>
      </div>
      {/* stretched over the whole row, under the cover's button */}
      <button
        type="button"
        onClick={() => onPick(found)}
        className="flex min-w-0 flex-1 cursor-pointer flex-col text-left outline-none after:absolute after:inset-0 after:rounded-md focus-visible:after:ring-1 focus-visible:after:ring-ring/50"
      >
        <span className={cn('truncate font-medium', status !== null && 'text-primary')}>{found.title}</span>
        <FoundCredits found={found} />
      </button>
      {found.durationMs !== null && (
        <span className="shrink-0 pr-2 text-sm text-muted-foreground tabular-nums">
          {formatClock(found.durationMs)}
        </span>
      )}
      <QuickDownload found={found} origin={origin} />
    </div>
  )
}

/** Downloads a found song at once with the tags it was found with, in the original quality, into the folder and playlist the dialog was opened from. While it runs, the row shows its progress and a cross that cancels it, as the downloads list does; then a check, or a mark naming the error that downloads it again on a click. It is left out for an upload that names no artist, as a download needs one. */
function QuickDownload({ found, origin }: { found: FoundSong; origin: Origin }) {
  const start = useMutation(mutations.startDownload())
  const { data: downloads = [] } = useQuery(queries.downloads())
  if (found.artists.length === 0) return null
  // the latest download of this song, also one started earlier or from its full form; a cancelled or dismissed one leaves the list, and the button comes back
  const download = downloads.findLast((d) => d.videoUrl === found.videoUrl)
  const startDownload = () =>
    start.mutate({
      videoUrl: found.videoUrl,
      cover: found.cover && { kind: 'web', url: found.cover.full },
      title: found.title,
      artists: found.artists,
      album: found.album,
      year: null,
      quality: 'original',
      folder: originFolder(origin),
      playlistIds: originPlaylists(origin),
    })
  // above the row's stretched pick button
  const above = 'relative z-10 flex shrink-0 items-center gap-1'
  if (download !== undefined && isActive(download.stage))
    return (
      <div className={above}>
        <DownloadProgress download={download} stage={download.stage} />
      </div>
    )
  const failed = download?.stage.kind === 'failed' ? download.stage.error : null
  const done = download?.stage.kind === 'done'
  const label = done ? 'Downloaded' : failed !== null ? `${errorMessage(failed)}. Click to try again` : 'Download now'
  return (
    <Hint label={label}>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        aria-label={label}
        disabled={start.isPending || done}
        className={above}
        onClick={startDownload}
      >
        {start.isPending ? (
          <LoaderCircleIcon className="animate-spin" />
        ) : done ? (
          <CheckIcon className="text-primary" />
        ) : failed !== null ? (
          <CircleAlertIcon className="text-destructive" />
        ) : (
          <DownloadIcon />
        )}
      </Button>
    </Hint>
  )
}

function FoundCover({ found, className }: { found: FoundSong; className?: string }) {
  return <SourceCover source={found.cover && { kind: 'web', url: found.cover.thumb }} className={className} />
}

/** The artists and the album of a found song in one muted line, when it names any. */
function FoundCredits({ found }: { found: FoundSong }) {
  const credits = [found.artists.join(', '), found.album].filter((part) => part !== null && part !== '').join(' · ')
  if (credits === '') return null
  return <span className="truncate text-sm text-muted-foreground">{credits}</span>
}

/** What a preview button shows: a spinner while the audio is found, then pause or play. */
function PreviewIcon({ status }: { status: PreviewStatus }) {
  switch (status) {
    case 'loading':
      return <LoaderCircleIcon className="animate-spin" />
    case 'playing':
      return <PauseIcon className="fill-current" />
    case 'paused':
      return <PlayIcon className="fill-current" />
    default:
      return status satisfies never
  }
}

/** The dialog's own small player along its bottom, shown once a found song is played: the song, play and pause, the songs before and after it among the results for `query`, the position and the app's volume. */
function PreviewBar({ query }: { query: string }) {
  const song = usePreview((state) => state.song)
  const status = usePreview((state) => state.status)
  const search = useQuery({
    ...queries.downloadSearch(query),
    enabled: query !== '',
    placeholderData: keepPreviousData,
  })
  if (song === null) return null
  const order = search.data === undefined ? [] : [...search.data.songs, ...search.data.videos]
  // a song from an earlier search is no longer in the list, so the next one is the first
  const index = order.findIndex((found) => found.videoUrl === song.videoUrl)
  const before = index > 0 ? order[index - 1] : undefined
  const after = order[index + 1]
  return (
    // the main player bar's layout: the song, then the controls over the seek bar, then the volume
    <DialogFooter className="mt-auto grid grid-cols-[minmax(0,30%)_minmax(0,40%)_minmax(0,30%)] items-center gap-0 px-4 py-3">
      <div className="flex min-w-0 items-center gap-3 pr-3">
        <FoundCover found={song} className="size-12" />
        <div className="flex min-w-0 flex-col">
          <Marquee play="always" className="text-sm font-medium">
            {song.title}
          </Marquee>
          {song.artists.length > 0 && (
            <Marquee play="always" className="text-xs text-muted-foreground">
              {song.artists.join(', ')}
            </Marquee>
          )}
        </div>
      </div>
      <div className="flex min-w-0 flex-col items-center gap-1">
        <div className="flex items-center gap-4">
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label="Previous"
            disabled={before === undefined}
            onClick={() => before !== undefined && void playPreview(before)}
          >
            <SkipBackIcon className="fill-current" />
          </Button>
          <Button
            type="button"
            variant="light"
            size="icon"
            aria-label={status === 'playing' ? 'Pause' : 'Play'}
            onClick={() => (status === 'playing' ? pausePreview() : void playPreview(song))}
          >
            <PreviewIcon status={status} />
          </Button>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label="Next"
            disabled={after === undefined}
            onClick={() => after !== undefined && void playPreview(after)}
          >
            <SkipForwardIcon className="fill-current" />
          </Button>
        </div>
        <PreviewSeek />
      </div>
      <div className="flex justify-end">
        <VolumeControl />
      </div>
    </DialogFooter>
  )
}

/** Where the preview has got, which a drag moves once it is let go. */
function PreviewSeek() {
  const positionMs = usePreview((state) => state.positionMs)
  const durationMs = usePreview((state) => state.durationMs)
  const [shown, setDraft] = useDraft(positionMs)
  const clock = 'w-10 text-xs text-muted-foreground tabular-nums'
  return (
    <div className="flex w-full min-w-0 items-center gap-2">
      <span className={cn(clock, 'text-right')}>{formatClock(shown)}</span>
      <ValueSlider
        aria-label="Preview position"
        disabled={durationMs === 0}
        max={Math.max(durationMs, 1)}
        step={1000}
        value={shown}
        onSlide={setDraft}
        onSlideEnd={(position) => {
          setDraft(null)
          seekPreview(position)
        }}
      />
      <span className={clock}>{formatClock(durationMs)}</span>
    </div>
  )
}

/** Pastes the clipboard's link with one click, showing whose link it is; disabled while the clipboard holds none. */
function PasteButton({ onPaste }: { onPaste: (link: string) => void }) {
  const { data: text = null } = useQuery({ ...queries.clipboardText(), refetchInterval: CLIPBOARD_POLL_MS })
  const link = text?.trim() ?? ''
  const service = linkService(link)
  return (
    <Button type="button" variant="outline" disabled={service === null} onClick={() => onPaste(link)}>
      {service === null ? <ClipboardIcon /> : <ServiceIcon service={service} className="size-4" />}
      Paste
    </Button>
  )
}

/** Shows the first-run setup of the download tools while it runs, then its children. */
function ToolsSetup({ failed, children }: { failed: string | undefined; children: ReactNode }) {
  const { data: tools } = useQuery(queries.downloadTools())
  const installing = tools?.installing
  if (failed !== undefined)
    return (
      <DialogBody className="pt-0">
        <Status icon={<CircleAlertIcon className="text-destructive" />}>Setting up downloads failed: {failed}</Status>
      </DialogBody>
    )
  if (installing == null) return children
  return (
    <DialogBody className="pt-0">
      <ToolInstallProgress install={installing} />
    </DialogBody>
  )
}

function Status({ icon, children }: { icon: ReactNode; children: ReactNode }) {
  return (
    <p className="flex items-center gap-2 text-sm text-muted-foreground [&_svg]:size-4 [&_svg]:shrink-0">
      {icon}
      <span>{children}</span>
    </p>
  )
}

/** The song about to be downloaded, its tags taken from the search result it was picked from, when it was, over those guessed from the video. */
function PreviewForm({
  preview,
  found,
  origin,
  onDone,
}: {
  preview: DownloadPreview
  found: FoundSong | null
  origin: Origin
  onDone: () => void
}) {
  const [title, setTitle] = useState(found?.title ?? preview.title)
  const [artists, setArtists] = useState(found !== null && found.artists.length > 0 ? found.artists : preview.artists)
  const [album, setAlbum] = useState(found?.album ?? preview.album ?? '')
  const [year, setYear] = useState(preview.year?.toString() ?? '')
  const coverUrl = found?.cover?.full ?? preview.coverUrl
  const [cover, setCover] = useState<CoverSource | null>(coverUrl === null ? null : { kind: 'web', url: coverUrl })
  const [quality, setQuality] = useState<AudioQuality>('original')
  const [folder, setFolder] = useState(originFolder(origin))
  const [playlistIds, setPlaylistIds] = useState(originPlaylists(origin))
  const start = useMutation(mutations.startDownload())
  const { data: check } = useQuery(queries.downloadFolder(folder))
  const yearValid = year === '' || YEAR_PATTERN.test(year)
  const blocked =
    title.trim() === '' || artists.length === 0 || !yearValid || check === undefined || check.kind === 'overlaps'
  const originalKbps = preview.qualities.find((option) => option.quality === 'original')?.kbps

  return (
    <form
      className="flex min-h-0 flex-1 flex-col"
      onSubmit={(event) => {
        event.preventDefault()
        start.mutate(
          {
            videoUrl: preview.videoUrl,
            cover,
            title,
            artists,
            album: album.trim() === '' ? null : album,
            year: year === '' ? null : Number(year),
            quality,
            folder,
            playlistIds,
          },
          { onSuccess: onDone },
        )
      }}
    >
      <DialogBody className="pt-0">
        <SongCard
          facts={
            <>
              <Fact label="Length">{formatClock(preview.durationMs)}</Fact>
              {originalKbps !== undefined && <Fact label="Audio">AAC {originalKbps} kbps</Fact>}
            </>
          }
        >
          <CoverPicker
            removable={cover !== null}
            onPick={(path) => setCover({ kind: 'file', path })}
            onRemove={() => setCover(null)}
            className="size-40"
          >
            <SourceCover source={cover} className="size-full shadow-lg" />
          </CoverPicker>
          <div className="grid min-w-0 flex-1 grid-cols-[1fr_5.5rem] content-start gap-3">
            <TextField
              label="Title"
              placeholder={preview.title}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="col-span-2"
            />
            <ArtistsField
              placeholder={preview.artists.join(', ')}
              value={artists}
              onValueChange={setArtists}
              className="col-span-2"
            />
            <AlbumField
              placeholder={preview.album ?? undefined}
              value={album}
              onValueChange={setAlbum}
              owners={artists.slice(0, 1)}
              onPick={(picked) => {
                setArtists(leading(picked.artists[0]?.name ?? null, artists))
                if (picked.year !== null) setYear(picked.year.toString())
              }}
            />
            <TextField
              label="Year"
              placeholder={preview.year?.toString()}
              inputMode="numeric"
              maxLength={4}
              value={year}
              aria-invalid={!yearValid}
              onChange={(e) => setYear(digitsOf(e.target.value))}
              className="[&_input]:tabular-nums"
            />
          </div>
        </SongCard>
        <Section title="Quality">
          <RadioGroup
            value={quality}
            onValueChange={(value) =>
              setQuality(preview.qualities.find((q) => q.quality === value)?.quality ?? quality)
            }
            className="grid-cols-4"
          >
            {preview.qualities.map((option) => (
              <RadioCard key={option.quality} value={option.quality} className="flex-col items-start gap-0.5">
                <span className="font-semibold">{qualityLabels[option.quality]}</span>
                <span className="text-xs text-muted-foreground tabular-nums">
                  {option.quality === 'original' ? `AAC ${option.kbps}` : option.kbps} kbps
                </span>
                <span className="text-xs text-muted-foreground tabular-nums">
                  ~{formatBytes(option.estimatedBytes)}
                </span>
              </RadioCard>
            ))}
          </RadioGroup>
        </Section>
        <PlaylistPicker selected={playlistIds} onChange={setPlaylistIds} />
        <Section title="Save to">
          <div className="flex items-center gap-3 rounded-md bg-secondary px-3 py-2 text-sm">
            <FolderIcon className="size-4 shrink-0 text-muted-foreground" />
            <span className="min-w-0 flex-1 truncate select-text">{check?.path ?? '…'}</span>
            {folder !== null && !('folder' in origin) && (
              <Button type="button" variant="ghost" size="sm" onClick={() => setFolder(null)}>
                Default
              </Button>
            )}
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={async () => {
                const picked = await open({
                  directory: true,
                  title: 'Save the song to',
                  ...(check && { defaultPath: check.path }),
                })
                if (picked !== null) setFolder(picked)
              }}
            >
              Change
            </Button>
          </div>
          {check !== undefined && <FolderNote check={check} />}
        </Section>
      </DialogBody>
      <DialogFooter>
        <DialogClose asChild>
          <Button type="button" variant="ghost">
            Cancel
          </Button>
        </DialogClose>
        <Button type="submit" variant="light" disabled={blocked || start.isPending}>
          Download
        </Button>
      </DialogFooter>
    </form>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="grid gap-2">
      <h3 className="text-sm font-semibold text-muted-foreground">{title}</h3>
      {children}
    </section>
  )
}

function FolderNote({ check }: { check: FolderCheck }) {
  switch (check.kind) {
    case 'inLibrary':
      return <Status icon={<CircleCheckIcon className="text-primary" />}>In your library</Status>
    case 'willBeAdded':
      return <Status icon={<FolderPlusIcon />}>This folder will be added to your library</Status>
    case 'overlaps':
      return (
        <Status icon={<CircleAlertIcon className="text-destructive" />}>
          This folder contains {check.existing}, which is already in your library; pick that folder or one inside it
        </Status>
      )
    default:
      return check satisfies never
  }
}

/** Tiles of every playlist, a big cover over its name, that pick the ones the song should be added to once it is downloaded; a picked tile is outlined in the accent colour. */
function PlaylistPicker({ selected, onChange }: { selected: PlaylistId[]; onChange: (ids: PlaylistId[]) => void }) {
  const playlists = useOrderedPlaylists()
  if (playlists.length === 0) return null
  return (
    <Section title="Add to playlists">
      <div className="grid max-h-80 grid-cols-[repeat(auto-fill,minmax(7.5rem,1fr))] gap-1 overflow-y-auto">
        {playlists.map((playlist) => {
          const picked = selected.includes(playlist.id)
          return (
            <button
              key={playlist.id}
              type="button"
              aria-pressed={picked}
              onClick={() =>
                onChange(picked ? selected.filter((id) => id !== playlist.id) : [...selected, playlist.id])
              }
              className="group flex min-w-0 cursor-pointer flex-col gap-2 rounded-lg p-2 text-left text-sm outline-1 -outline-offset-1 outline-transparent transition-[outline-color] hover:bg-surface-hover focus-visible:bg-surface-hover aria-pressed:outline-primary"
            >
              <Cover src={playlist.cover} kind="playlist" className="w-full" />
              <span className="truncate font-medium group-aria-pressed:text-primary">{playlist.name}</span>
            </button>
          )
        })}
      </div>
    </Section>
  )
}
