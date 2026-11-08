import { useMutation, useSuspenseQuery } from '@tanstack/react-query'
import { Link, createFileRoute } from '@tanstack/react-router'
import { open } from '@tauri-apps/plugin-dialog'
import { FolderIcon, FolderPlusIcon, RefreshCwIcon, Trash2Icon } from 'lucide-react'
import { useState } from 'react'
import type { Folder } from '@/bindings'
import { Hint } from '@/components/hint'
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
import { useScan } from '@/lib/backend'
import { plural } from '@/lib/format'
import { mutations, queries } from '@/lib/query'

export const Route = createFileRoute('/settings/folders')({
  loader: ({ context }) => context.queryClient.ensureQueryData(queries.folders()),
  component: Folders,
})

function Folders() {
  const { data: folders } = useSuspenseQuery(queries.folders())
  const addFolder = useMutation(mutations.addFolder())
  const [removing, setRemoving] = useState<Folder | null>(null)
  const scan = useMutation(mutations.scan())
  const busy = addFolder.isPending || scan.isPending

  return (
    <div className="flex max-w-3xl flex-col gap-6 p-6">
      <header className="flex flex-col gap-2">
        <h1 className="text-3xl font-bold">Folders</h1>
        <p className="text-muted-foreground">
          Aio reads every audio file inside these folders and their subfolders. A folder cannot sit inside another one
          that is already here.
        </p>
      </header>
      <div className="flex gap-2">
        <Button
          disabled={busy}
          onClick={async () => {
            const path = await open({ directory: true, title: 'Add a music folder' })
            if (path !== null) addFolder.mutate(path)
          }}
        >
          <FolderPlusIcon />
          Add folder
        </Button>
        <Button variant="outline" disabled={busy || folders.length === 0} onClick={() => scan.mutate()}>
          <RefreshCwIcon className={busy ? 'animate-spin' : undefined} />
          Rescan
        </Button>
      </div>
      <ScanStatus />
      <ul className="flex flex-col gap-1">
        {folders.map((folder) => (
          <li key={folder.id} className="group flex items-center gap-4 rounded-md p-3 hover:bg-surface-hover">
            <Link
              to="/folders/$folderId"
              params={{ folderId: folder.id }}
              className="flex min-w-0 flex-1 items-center gap-4"
            >
              <FolderIcon className="size-6 shrink-0 text-muted-foreground" />
              <div className="flex min-w-0 flex-1 flex-col">
                <span className="truncate">{folder.path}</span>
                <span className="text-sm text-muted-foreground">{plural(folder.trackCount, 'song')}</span>
              </div>
            </Link>
            <Hint label="Remove from library">
              <Button
                variant="ghost"
                size="icon"
                disabled={busy}
                className="opacity-0 group-hover:opacity-100"
                onClick={() => setRemoving(folder)}
              >
                <Trash2Icon />
              </Button>
            </Hint>
          </li>
        ))}
      </ul>
      <Dialog open={removing !== null} onOpenChange={() => setRemoving(null)}>
        {removing !== null && <RemoveConfirmation folder={removing} onDone={() => setRemoving(null)} />}
      </Dialog>
    </div>
  )
}

function RemoveConfirmation({ folder, onDone }: { folder: Folder; onDone: () => void }) {
  const removeFolder = useMutation(mutations.removeFolder())
  return (
    <DialogContent>
      <DialogHeader>
        <DialogTitle>Remove folder?</DialogTitle>
      </DialogHeader>
      <DialogBody>
        <DialogDescription>
          <span className="font-semibold break-all text-foreground">{folder.path}</span> and its{' '}
          {plural(folder.trackCount, 'song')} leave the library. The files on disk stay where they are, and songs of
          this folder in your playlists show as missing until you add the folder back.
        </DialogDescription>
      </DialogBody>
      <DialogFooter>
        <DialogClose asChild>
          <Button variant="ghost">Cancel</Button>
        </DialogClose>
        <Button
          variant="destructive"
          disabled={removeFolder.isPending}
          onClick={() => removeFolder.mutate(folder.path, { onSuccess: onDone })}
        >
          Remove
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}

function ScanStatus() {
  const progress = useScan()
  if (progress === null) return null
  const percent = progress.total === 0 ? 0 : (progress.done / progress.total) * 100
  return (
    <div className="flex flex-col gap-2">
      <span className="text-sm text-muted-foreground">
        Reading {progress.done.toLocaleString()} of {plural(progress.total, 'file')}
      </span>
      <div className="h-1 overflow-hidden rounded-full bg-foreground/20">
        <div className="h-full bg-primary transition-[width]" style={{ width: `${percent}%` }} />
      </div>
    </div>
  )
}
