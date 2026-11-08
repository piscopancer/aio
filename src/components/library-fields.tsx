import { useQuery } from '@tanstack/react-query'
import { XIcon } from 'lucide-react'
import { useRef, useState } from 'react'
import { type Album, UNKNOWN_ALBUM } from '@/bindings'
import { Cover } from '@/components/cover'
import { Combobox } from '@/components/ui/combobox'
import { plural } from '@/lib/format'
import { queries } from '@/lib/query'
import { suggest } from '@/lib/search'

/** How many suggestions the artist and album fields show. */
const SUGGESTIONS = 8

/** The library album a song with this album title and owning artist is filed under, if it exists. */
export const findAlbum = (albums: readonly Album[], title: string, owner: string) =>
  albums.find((item) => item.title === (title.trim() || UNKNOWN_ALBUM) && item.artist === owner)

/** The artists with `main` moved or added to the front, as when a picked album names whose it is. */
export const leading = (main: string, artists: readonly string[]) => [
  main,
  ...artists.filter((name) => name.toLowerCase() !== main.toLowerCase()),
]

/** Characters that end a typed artist name and start the next one. */
const NAME_BREAK = ','

/** Artists field of a song: the credited artists as chips in order, the main one first, that can be dragged into another order or removed, and a text field under them that suggests the library's artists and adds the typed name on comma, Enter or leaving the field. */
export function ArtistsField({
  value,
  onValueChange,
  placeholder,
  className,
}: {
  value: readonly string[]
  onValueChange: (value: string[]) => void
  placeholder?: string | undefined
  className?: string
}) {
  const { data: artists = [] } = useQuery(queries.artists())
  const [draft, setDraft] = useState('')
  const dragged = useRef<number | null>(null)
  const coverOf = (name: string) => artists.find((item) => item.name === name.trim())?.cover ?? null
  const listed = (name: string) => value.some((known) => known.toLowerCase() === name.trim().toLowerCase())
  const options = suggest(
    draft,
    artists.filter((item) => !listed(item.name)),
    SUGGESTIONS,
    (item) => item.name,
  ).map((item) => ({
    key: item.id,
    label: item.name,
    subtitle: plural(item.trackCount, 'song'),
    art: <Cover src={item.cover} kind="artist" />,
  }))

  const add = (names: readonly string[]) => {
    const fresh = names.map((name) => name.trim()).filter((name) => name !== '' && !listed(name))
    if (fresh.length > 0) onValueChange([...value, ...fresh])
  }
  const commit = () => {
    add([draft])
    setDraft('')
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
      label="Artists"
      placeholder={value.length === 0 ? placeholder : 'Add artist'}
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
        if (event.key === 'Enter' && options.length === 0 && draft.trim() !== '') {
          // the name goes in as a chip instead of submitting the form
          event.preventDefault()
          commit()
        } else if (event.key === 'Backspace' && draft === '' && value.length > 0) {
          event.preventDefault()
          onValueChange(value.slice(0, -1))
        }
      }}
      onBlur={commit}
      // a name not in the library yet still gets a blank picture, as the artist it will create
      art={draft.trim() === '' ? undefined : <Cover src={coverOf(draft)} kind="artist" />}
      className={className}
    >
      {value.length > 0 && (
        <ul className="flex flex-wrap gap-1.5">
          {value.map((name, index) => (
            <li
              key={name}
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
              className="flex cursor-grab items-center gap-1.5 rounded-full bg-foreground/10 py-0.5 pr-0.5 pl-0.5 text-sm active:cursor-grabbing"
            >
              <Cover src={coverOf(name)} kind="artist" className="size-6" />
              <span className="max-w-48 truncate">{name}</span>
              <button
                type="button"
                aria-label={`Remove ${name}`}
                onClick={() => onValueChange(value.filter((_, at) => at !== index))}
                className="flex size-6 cursor-pointer items-center justify-center rounded-full text-muted-foreground hover:bg-foreground/10 hover:text-foreground"
              >
                <XIcon className="size-3.5" />
              </button>
            </li>
          ))}
        </ul>
      )}
    </Combobox>
  )
}

/** Album field that suggests the library's named albums, only those of `owner` once it is given, with the cover of the one the text names. */
export function AlbumField({
  value,
  onValueChange,
  owner,
  onPick,
  placeholder,
  className,
}: {
  value: string
  onValueChange: (value: string) => void
  /** The album artist, or else the artist, the album belongs to. */
  owner: string
  onPick: (album: Album) => void
  placeholder?: string | undefined
  className?: string
}) {
  const { data: albums = [] } = useQuery(queries.albums())
  const target = findAlbum(albums, value, owner)
  const candidates = albums.filter((item) => item.title !== UNKNOWN_ALBUM && (owner === '' || item.artist === owner))
  const options = (
    value.trim() === ''
      ? candidates.slice(0, SUGGESTIONS)
      : suggest(value, candidates, SUGGESTIONS, (item) => item.title)
  ).map((item) => ({
    key: item.id,
    label: item.title,
    subtitle: [item.artist, item.year].filter((part) => part !== null).join(' • '),
    art: <Cover src={item.cover} kind="album" className="rounded-sm" />,
    album: item,
  }))
  return (
    <Combobox
      label="Album"
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
