import { XIcon } from 'lucide-react'
import { cn } from '@/lib/utils'

/** Cross at the end of a text field that empties it and puts the cursor in it. */
export function ClearButton({ onClear, className }: { onClear: () => void; className?: string }) {
  return (
    <button
      type="button"
      aria-label="Clear"
      tabIndex={-1}
      onMouseDown={(event) => event.preventDefault()}
      onClick={(event) => {
        onClear()
        // the field the cross sits in takes the focus, ready for new text
        event.currentTarget.parentElement?.querySelector('input')?.focus()
      }}
      className={cn(
        'absolute right-1.5 flex size-7 cursor-pointer items-center justify-center rounded-full text-muted-foreground hover:bg-foreground/10 hover:text-foreground',
        className,
      )}
    >
      <XIcon className="size-4" />
    </button>
  )
}
