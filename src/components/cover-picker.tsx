import { useMutation } from '@tanstack/react-query'
import { open } from '@tauri-apps/plugin-dialog'
import { ImageIcon, LoaderCircleIcon, XIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'
import { mutations } from '@/lib/query'
import { cn } from '@/lib/utils'

/** Image formats the picker offers, all of which the backend decodes. */
const IMAGE_EXTENSIONS = ['jpg', 'jpeg', 'png', 'webp']

/** Square artwork that picks an image file on click and stores a cropped copy, with a cross that drops the image. */
export function CoverPicker({
  removable,
  onPick,
  onRemove,
  className,
  children,
}: {
  removable: boolean
  /** Receives the path of the stored square copy. */
  onPick: (path: string) => void
  onRemove: () => void
  className?: string
  /** The artwork shown now. */
  children: ReactNode
}) {
  const importCover = useMutation(mutations.importCover())
  return (
    <div className={cn('group relative aspect-square shrink-0', className)}>
      <button
        type="button"
        aria-label="Choose image"
        disabled={importCover.isPending}
        className="size-full cursor-pointer rounded-md outline-none focus-visible:ring-1 focus-visible:ring-ring/50"
        onClick={async () => {
          const file = await open({
            title: 'Choose an image',
            filters: [{ name: 'Images', extensions: IMAGE_EXTENSIONS }],
          })
          if (file !== null) importCover.mutate(file, { onSuccess: onPick })
        }}
      >
        {children}
        <span
          data-busy={importCover.isPending}
          className="absolute inset-0 flex flex-col items-center justify-center gap-2 rounded-md bg-black/60 text-sm font-semibold opacity-0 transition-opacity group-hover:opacity-100 group-has-focus-visible:opacity-100 data-[busy=true]:opacity-100"
        >
          {importCover.isPending ? (
            <LoaderCircleIcon className="size-8 animate-spin" />
          ) : (
            <ImageIcon className="size-8" strokeWidth={1.5} />
          )}
          Choose image
        </span>
      </button>
      {removable && (
        <Button
          type="button"
          size="icon"
          aria-label="Remove image"
          className="absolute top-2 right-2 bg-black/70 text-foreground opacity-0 group-hover:opacity-100 hover:bg-black focus-visible:opacity-100"
          onClick={onRemove}
        >
          <XIcon />
        </Button>
      )}
    </div>
  )
}
