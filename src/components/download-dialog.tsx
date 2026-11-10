import { useMutation, useQuery } from '@tanstack/react-query'
import { open } from '@tauri-apps/plugin-dialog'
import {
  CircleAlertIcon,
  CircleCheckIcon,
  ClipboardIcon,
  FolderIcon,
  FolderPlusIcon,
  LoaderCircleIcon,
  XIcon,
} from 'lucide-react'
import { type ReactNode, useEffect, useState } from 'react'
import type { AudioQuality, CoverSource, DownloadPreview, FolderCheck, PlaylistId } from '@/bindings'
import { Cover, SourceCover } from '@/components/cover'
import { CoverPicker } from '@/components/cover-picker'
import { ServiceIcon, ServiceStack, linkService, serviceNames } from '@/components/service-icon'
import { AlbumField, ArtistsField, leading } from '@/components/library-fields'
import { Fact, SongCard, YEAR_PATTERN, digitsOf } from '@/components/song-card'
import { ToolInstallProgress } from '@/components/tool-install'
import { Button } from '@/components/ui/button'
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
import { useOrderedPlaylists } from '@/lib/playlists'
import { mutations, queries } from '@/lib/query'

/** Pause after the last key press before a typed link is looked up; a pasted link is looked up at once. */
const LOOKUP_DELAY_MS = 400

/** How often the paste button checks the clipboard for a song link while the dialog waits for one. */
const CLIPBOARD_POLL_MS = 1000

/** Labels of the qualities the preview offers. */
const qualityLabels = {
  original: 'Original',
  'mp3-320': 'MP3 320',
  'mp3-192': 'MP3 192',
  'mp3-128': 'MP3 128',
} as const satisfies Record<AudioQuality, string>

/** Where the dialog was opened from: the playlist it pre-checks, or the folder it saves into. */
type Origin = { playlistId: PlaylistId } | { folder: string }

/** Dialog that downloads one song from a pasted link, opened by the given trigger. */
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
  const [link, setLink] = useState('')
  const [target, setTarget] = useState<string | null>(null)
  const settledLink = useDebounced(link.trim(), LOOKUP_DELAY_MS)
  // a typed link is looked up, and locked, once typing pauses
  useEffect(() => {
    if (linkService(settledLink) !== null) setTarget(settledLink)
  }, [settledLink])
  const lookUp = (text: string) => {
    setLink(text)
    setTarget(text)
  }
  const prepare = useQuery(queries.prepareDownloads())
  const preview = useQuery({ ...queries.downloadPreview(target ?? ''), enabled: target !== null })
  const service = linkService(target ?? link)

  return (
    <DialogContent className="max-w-xl">
      <DialogHeader>
        <div className="flex items-center gap-3">
          <DialogTitle>Download a song</DialogTitle>
          <ServiceStack />
        </div>
        {/* the marks beside the title say it to the eye, this to screen readers */}
        <DialogDescription className="sr-only">
          Paste a link to a song on YouTube, YouTube Music or Spotify.
        </DialogDescription>
      </DialogHeader>
      <div className="flex shrink-0 items-center gap-2 px-6 py-6">
        <label className="relative flex min-w-0 flex-1 items-center">
          <ServiceIcon service={service} className="pointer-events-none absolute left-3" />
          <Input
            autoFocus
            aria-label="Song link"
            placeholder="https://"
            value={link}
            readOnly={target !== null}
            onChange={(event) => setLink(event.target.value)}
            onPaste={(event) => {
              const text = event.clipboardData.getData('text').trim()
              if (linkService(text) === null) return
              event.preventDefault()
              lookUp(text)
            }}
            className="pl-11 read-only:text-muted-foreground"
          />
        </label>
        {target === null ? (
          <PasteButton onPaste={lookUp} />
        ) : (
          <Button
            type="button"
            variant="outline"
            onClick={() => {
              setLink('')
              setTarget(null)
            }}
          >
            <XIcon />
            Clear
          </Button>
        )}
      </div>
      {target !== null && service !== null && (
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
              <PreviewForm key={preview.data.videoUrl} preview={preview.data} origin={origin} onDone={onDone} />
            )
          )}
        </ToolsSetup>
      )}
    </DialogContent>
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

function PreviewForm({ preview, origin, onDone }: { preview: DownloadPreview; origin: Origin; onDone: () => void }) {
  const [title, setTitle] = useState(preview.title)
  const [artists, setArtists] = useState(preview.artists)
  const [album, setAlbum] = useState(preview.album ?? '')
  const [year, setYear] = useState(preview.year?.toString() ?? '')
  const [cover, setCover] = useState<CoverSource | null>(
    preview.coverUrl === null ? null : { kind: 'web', url: preview.coverUrl },
  )
  const [quality, setQuality] = useState<AudioQuality>('original')
  const [folder, setFolder] = useState('folder' in origin ? origin.folder : null)
  const [playlistIds, setPlaylistIds] = useState('playlistId' in origin ? [origin.playlistId] : [])
  const start = useMutation(mutations.startDownload())
  const { data: check } = useQuery(queries.downloadFolder(folder))
  const yearValid = year === '' || YEAR_PATTERN.test(year)
  const blocked =
    title.trim() === '' || artists.length === 0 || !yearValid || check === undefined || check.kind === 'overlaps'
  const originalKbps = preview.qualities.find((option) => option.quality === 'original')?.kbps

  return (
    <form
      className="flex min-h-0 flex-col"
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
