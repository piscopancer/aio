import { useMutation } from '@tanstack/react-query'
import { useNavigate, useRouter } from '@tanstack/react-router'
import { useState } from 'react'
import { PLAYLIST_NAME_MAX_CHARS, type Playlist } from '@/bindings'
import { Cover } from '@/components/cover'
import { CoverPicker } from '@/components/cover-picker'
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
} from '@/components/ui/dialog'
import { TextField } from '@/components/ui/text-field'
import { useNextPlaylistName } from '@/lib/playlists'
import { mutations } from '@/lib/query'

/** Which playlist dialog is open, if any. */
export type PlaylistDialog = 'edit' | 'delete' | null

/** What the details form edits: the name and the picked image, null when the songs' art stands in. */
type Details = Pick<Playlist, 'name' | 'ownCover'>

/** Edit and delete dialogs of a playlist, shared by its page and its sidebar menu. */
export function PlaylistDialogs({
  playlist,
  open,
  onClose,
}: {
  playlist: Playlist
  open: PlaylistDialog
  onClose: () => void
}) {
  return (
    <>
      <Dialog open={open === 'edit'} onOpenChange={onClose}>
        {open === 'edit' && <EditForm playlist={playlist} onDone={onClose} />}
      </Dialog>
      <Dialog open={open === 'delete'} onOpenChange={onClose}>
        <DeleteConfirmation playlist={playlist} onDone={onClose} />
      </Dialog>
    </>
  )
}

/** Dialog that creates a playlist from a name and an optional image, then opens its page. */
export function CreatePlaylistDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open && <CreateForm onDone={() => onOpenChange(false)} />}
    </Dialog>
  )
}

function CreateForm({ onDone }: { onDone: () => void }) {
  const name = useNextPlaylistName()
  const create = useMutation(mutations.createPlaylist())
  const navigate = useNavigate()
  return (
    <DetailsForm
      title="Create playlist"
      initial={{ name, ownCover: null }}
      fallbackCover={null}
      submitLabel="Create"
      pending={create.isPending}
      onSubmit={(details) =>
        create.mutate(details, {
          onSuccess: async (id) => {
            onDone()
            await navigate({ to: '/playlists/$playlistId', params: { playlistId: id } })
          },
        })
      }
    />
  )
}

function EditForm({ playlist, onDone }: { playlist: Playlist; onDone: () => void }) {
  const edit = useMutation(mutations.editPlaylist())
  return (
    <DetailsForm
      title="Edit details"
      initial={playlist}
      // the songs' art is only known while no image of its own hides it
      fallbackCover={playlist.ownCover === null ? playlist.cover : null}
      submitLabel="Save"
      pending={edit.isPending}
      onSubmit={(details) => edit.mutate({ id: playlist.id, ...details }, { onSuccess: onDone })}
    />
  )
}

/** Cover picker and name field of a playlist, shared by the create and edit dialogs. */
function DetailsForm({
  title,
  initial,
  fallbackCover,
  submitLabel,
  pending,
  onSubmit,
}: {
  title: string
  initial: Details
  fallbackCover: string | null
  submitLabel: string
  pending: boolean
  onSubmit: (details: Details) => void
}) {
  const [name, setName] = useState(initial.name)
  const [ownCover, setOwnCover] = useState(initial.ownCover)
  return (
    <DialogContent className="max-w-lg">
      <DialogHeader>
        <DialogTitle>{title}</DialogTitle>
      </DialogHeader>
      <form
        className="flex min-h-0 flex-col"
        onSubmit={(event) => {
          event.preventDefault()
          onSubmit({ name, ownCover })
        }}
      >
        <DialogBody className="flex-row items-start">
          <CoverPicker
            removable={ownCover !== null}
            onPick={setOwnCover}
            onRemove={() => setOwnCover(null)}
            className="size-44"
          >
            <Cover src={ownCover ?? fallbackCover} kind="playlist" className="size-full shadow-lg" />
          </CoverPicker>
          <div className="flex min-w-0 flex-1 flex-col">
            <TextField
              label="Name"
              autoFocus
              value={name}
              maxLength={PLAYLIST_NAME_MAX_CHARS}
              onChange={(event) => setName(event.target.value)}
              onFocus={(event) => event.target.select()}
            />
          </div>
        </DialogBody>
        <DialogFooter>
          <DialogClose asChild>
            <Button type="button" variant="ghost">
              Cancel
            </Button>
          </DialogClose>
          <Button type="submit" variant="light" disabled={name.trim() === '' || pending}>
            {submitLabel}
          </Button>
        </DialogFooter>
      </form>
    </DialogContent>
  )
}

/** Leaves the playlist page when the deleted playlist is the one on screen. */
function DeleteConfirmation({ playlist, onDone }: { playlist: Playlist; onDone: () => void }) {
  const navigate = useNavigate()
  // the router itself, read when the deletion lands, rather than a match hook, whose subscription would render every playlist's hidden dialog on every navigation
  const router = useRouter()
  const remove = useMutation({
    ...mutations.deletePlaylist(),
    // on the mutation rather than on `mutate`, so it still runs once the deleted playlist has left the sidebar along with this dialog, and before the playlists are fetched again, while the page would still ask for the one gone
    onSuccess: async (_, id) => {
      // the matches hold the parsed id, where `matchRoute` would compare the number with the raw text of the address
      const onPage = router.state.matches.some(
        (match) => match.routeId === '/playlists/$playlistId' && match.params.playlistId === id,
      )
      if (onPage) await navigate({ to: '/' })
    },
  })
  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Delete playlist?</DialogTitle>
      </DialogHeader>
      <DialogBody>
        <DialogDescription>
          This deletes <span className="font-semibold text-foreground">{playlist.name}</span>. The songs stay in your
          library.
        </DialogDescription>
      </DialogBody>
      <DialogFooter>
        <DialogClose asChild>
          <Button variant="ghost">Cancel</Button>
        </DialogClose>
        <Button
          variant="destructive"
          disabled={remove.isPending}
          onClick={() => remove.mutate(playlist.id, { onSuccess: onDone })}
        >
          Delete
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}
