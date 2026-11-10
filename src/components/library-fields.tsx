import { useQuery } from '@tanstack/react-query'
import { Disc3Icon, MicVocalIcon, PencilIcon, TagsIcon, XIcon } from 'lucide-react'
import { type ComponentProps, useRef, useState } from 'react'
import type { Album } from '@/bindings'
import { ChipLabel } from '@/components/chip-label'
import { Cover } from '@/components/cover'
import { Combobox } from '@/components/ui/combobox'
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from '@/components/ui/context-menu'
import type { FieldLabelProps } from '@/components/ui/field-label'
import { credit, plural } from '@/lib/format'
import { queries } from '@/lib/query'
import { suggest } from '@/lib/search'
import { cn } from '@/lib/utils'

/** How many suggestions the artist, album and genre fields show. */
const SUGGESTIONS = 8

/** Whether two lists name the same values in the same order. */
export const sameNames = (a: readonly string[], b: readonly string[]) =>
  a.length === b.length && a.every((name, index) => name === b[index])

/** Whether the album is filed under exactly these artists, in this order. */
export const ownedBy = (album: Album, owners: readonly string[]) =>
  sameNames(
    album.artists.map((artist) => artist.name),
    owners,
  )

/** The library album a song with this album title and owning artists is filed under, if it exists; a song without an album title is on none, and one without owners is on the album of nobody by that title. */
export const findAlbum = (albums: readonly Album[], title: string, owners: readonly string[]) =>
  title.trim() === '' ? undefined : albums.find((item) => item.title === title.trim() && ownedBy(item, owners))

/** The artists with `main` moved or added to the front, as when a picked album names whose it is; an album of nobody
 * leaves them as they are. */
export const leading = (main: string | null, artists: readonly string[]) =>
  main === null ? [...artists] : [main, ...artists.filter((name) => name.toLowerCase() !== main.toLowerCase())]

/** The album artists a song names apart from its main artist `main`: the main artist alone names none, as the backend stores it. */
export const ownAlbumArtists = (albumArtists: readonly string[], main: string | null) =>
  main !== null && sameNames(albumArtists, [main]) ? [] : albumArtists

/** Characters that end a typed name and start the next one. */
const NAME_BREAK = ','

/** One value the library already knows, offered while typing. */
type Known = { key: string | number; label: string; subtitle: string; cover?: string | null }

/** A list of names or words as chips in order, the first one leading, that can be dragged into another order, removed, or taken back into the text field to be edited by a double click, from their context menu or by dropping them on it, and a text field under them that suggests the `known` values and adds the typed one on comma, Enter or leaving the field; with `kind`, every chip and suggestion shows its picture. */
function ChipsField({
  label,
  icon,
  hint,
  value,
  onValueChange,
  known,
  kind,
  placeholder,
  className,
}: FieldLabelProps & {
  label: string
  value: readonly string[]
  onValueChange: (value: string[]) => void
  known: readonly Known[]
  kind?: ComponentProps<typeof Cover>['kind']
  placeholder?: string | undefined
  className?: string
}) {
  const [draft, setDraft] = useState('')
  const dragged = useRef<number | null>(null)
  const input = useRef<HTMLInputElement>(null)
  /** Where a chip taken back into the text field returns once its name is added again. */
  const returning = useRef<number | null>(null)
  const picture = (name: string, className?: string) =>
    kind === undefined ? undefined : (
      <Cover src={known.find((item) => item.label === name.trim())?.cover ?? null} kind={kind} className={className} />
    )
  const listed = (name: string) => value.some((item) => item.toLowerCase() === name.trim().toLowerCase())
  const options = (
    draft.trim() === ''
      ? known.filter((item) => !listed(item.label)).slice(0, SUGGESTIONS)
      : suggest(
          draft,
          known.filter((item) => !listed(item.label)),
          SUGGESTIONS,
          (item) => item.label,
        )
  ).map((item) => ({ key: item.key, label: item.label, subtitle: item.subtitle, art: picture(item.label) }))

  const add = (names: readonly string[]) => {
    const fresh = names.map((name) => name.trim()).filter((name) => name !== '' && !listed(name))
    if (fresh.length === 0) return
    const at = returning.current ?? value.length
    returning.current = null
    onValueChange(value.toSpliced(at, 0, ...fresh))
  }
  const commit = () => {
    add([draft])
    setDraft('')
    // an edited name cleared away stays removed, and the next one typed goes last again
    returning.current = null
  }
  const remove = (index: number) => onValueChange(value.filter((_, at) => at !== index))
  /** Takes the chip at `index` back into the text field, adding whatever was typed there first so it is not lost. */
  const edit = (index: number) => {
    const name = value[index]
    if (name === undefined) return
    const rest = value.filter((_, at) => at !== index)
    const typed = draft.trim()
    const kept =
      typed === '' || rest.some((item) => item.toLowerCase() === typed.toLowerCase()) ? rest : [...rest, typed]
    onValueChange(kept)
    returning.current = Math.min(index, kept.length)
    setDraft(name)
  }
  /** Edits the chip at `index` from a double click or a drop, the field taking the focus at once. */
  const editHere = (index: number) => {
    edit(index)
    input.current?.focus()
  }
  const move = (from: number, to: number) => {
    const next = [...value]
    const [name] = next.splice(from, 1)
    if (name === undefined) return
    next.splice(to, 0, name)
    onValueChange(next)
  }

  return (
    <Combobox
      label={label}
      icon={icon}
      hint={hint}
      placeholder={value.length === 0 ? placeholder : 'Add more'}
      value={draft}
      onValueChange={(text) => {
        // a typed or pasted comma closes every name before it
        const parts = text.split(NAME_BREAK)
        const rest = parts.pop() ?? ''
        add(parts)
        setDraft(rest)
      }}
      options={options}
      onPick={(option) => {
        add([option.label])
        setDraft('')
      }}
      onKeyDown={(event) => {
        // a highlighted suggestion has taken Enter before this
        if (event.key === 'Enter' && draft.trim() !== '') {
          // the name goes in as a chip instead of submitting the form
          event.preventDefault()
          commit()
        } else if (event.key === 'Backspace' && draft === '' && value.length > 0) {
          event.preventDefault()
          onValueChange(value.slice(0, -1))
        }
      }}
      onBlur={commit}
      ref={input}
      onDragOver={(event) => {
        if (dragged.current === null) return
        event.preventDefault()
        event.dataTransfer.dropEffect = 'move'
      }}
      onDrop={(event) => {
        if (dragged.current === null) return
        event.preventDefault()
        editHere(dragged.current)
        dragged.current = null
      }}
      // a name not in the library yet still gets a blank picture, as what it will create
      art={draft.trim() === '' ? undefined : picture(draft)}
      className={className}
    >
      {value.length > 0 && (
        <ul className="flex flex-wrap gap-1.5">
          {value.map((name, index) => (
            <ContextMenu key={name}>
              <ContextMenuTrigger asChild>
                <li
                  draggable
                  onDragStart={(event) => {
                    dragged.current = index
                    event.dataTransfer.effectAllowed = 'move'
                  }}
                  onDragOver={(event) => {
                    if (dragged.current === null) return
                    event.preventDefault()
                    if (dragged.current !== index) {
                      move(dragged.current, index)
                      dragged.current = index
                    }
                  }}
                  onDragEnd={() => (dragged.current = null)}
                  onDoubleClick={() => editHere(index)}
                  className={cn(
                    'flex cursor-grab items-center gap-1.5 rounded-full bg-foreground/10 py-0.5 pr-0.5 text-sm active:cursor-grabbing',
                    kind === undefined ? 'pl-2.5' : 'pl-0.5',
                  )}
                >
                  {picture(name, 'size-6')}
                  <ChipLabel>{name}</ChipLabel>
                  <button
                    type="button"
                    aria-label={`Remove ${name}`}
                    onClick={() => remove(index)}
                    className="flex size-6 cursor-pointer items-center justify-center rounded-full text-muted-foreground hover:bg-foreground/10 hover:text-foreground"
                  >
                    <XIcon className="size-3.5" />
                  </button>
                </li>
              </ContextMenuTrigger>
              {/* the field takes the focus only once the menu is gone, as the open menu keeps it inside */}
              <ContextMenuContent
                onCloseAutoFocus={(event) => {
                  event.preventDefault()
                  if (returning.current !== null) input.current?.focus()
                }}
              >
                <ContextMenuItem onSelect={() => edit(index)}>
                  <PencilIcon />
                  Edit
                </ContextMenuItem>
                <ContextMenuItem onSelect={() => remove(index)}>
                  <XIcon />
                  Remove
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          ))}
        </ul>
      )}
    </Combobox>
  )
}

/** Field of people as chips in order, the first one leading, that suggests the library's artists with their pictures; the credited artists of a song by default, and also its album artists and writers. */
export function ArtistsField({
  label = 'Artists',
  icon = MicVocalIcon,
  ...props
}: Omit<ComponentProps<typeof ChipsField>, 'label' | 'known' | 'kind'> & { label?: string }) {
  const { data: artists = [] } = useQuery(queries.artists())
  const known = artists.map((item) => ({
    key: item.id,
    label: item.name,
    subtitle: plural(item.trackCount, 'song'),
    cover: item.cover,
  }))
  return <ChipsField {...props} label={label} icon={icon} known={known} kind="artist" />
}

/** Genres of a song as chips, suggesting the genres the library's songs already name, the most common first. */
export function GenresField({
  icon = TagsIcon,
  ...props
}: Omit<ComponentProps<typeof ChipsField>, 'label' | 'known' | 'kind'>) {
  const { data: genres = [] } = useQuery(queries.genres())
  const known = genres.map((item) => ({ key: item.name, label: item.name, subtitle: plural(item.trackCount, 'song') }))
  return <ChipsField {...props} label="Genres" icon={icon} known={known} />
}

/** Album field that suggests the library's named albums, only those of `owners` once they are given, with the cover of the one the text names. */
export function AlbumField({
  label = 'Album',
  icon = Disc3Icon,
  hint,
  value,
  onValueChange,
  owners,
  onPick,
  placeholder,
  className,
}: FieldLabelProps & {
  label?: string
  value: string
  onValueChange: (value: string) => void
  /** Whose albums are offered: the album artists, or else the artist, the album belongs to; with none yet, every album is. */
  owners: readonly string[]
  onPick: (album: Album) => void
  placeholder?: string | undefined
  className?: string
}) {
  const { data: albums = [] } = useQuery(queries.albums())
  const target = findAlbum(albums, value, owners)
  const candidates = albums.filter((item) => owners.length === 0 || ownedBy(item, owners))
  const options = (
    value.trim() === ''
      ? candidates.slice(0, SUGGESTIONS)
      : suggest(value, candidates, SUGGESTIONS, (item) => item.title)
  ).map((item) => ({
    key: item.id,
    label: item.title,
    subtitle: [credit(item.artists), item.year].filter((part) => part !== null && part !== '').join(' • '),
    art: <Cover src={item.cover} kind="album" className="rounded-sm" />,
    album: item,
  }))
  return (
    <Combobox
      label={label}
      icon={icon}
      hint={hint}
      placeholder={placeholder}
      value={value}
      onValueChange={onValueChange}
      options={options}
      onPick={(option) => {
        onValueChange(option.album.title)
        onPick(option.album)
      }}
      art={value.trim() === '' ? undefined : <Cover src={target?.cover ?? null} kind="album" className="rounded-sm" />}
      className={className}
    />
  )
}
