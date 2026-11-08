import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

/** A keyboard shortcut drawn as a row of key caps, such as `keys={['Ctrl', 'F']}`. */
export function Shortcut<const Keys extends readonly string[]>({
  keys,
  className,
  ...props
}: Omit<ComponentProps<'kbd'>, 'children'> & { keys: Keys }) {
  return (
    <kbd className={cn('flex items-center gap-1 font-sans', className)} {...props}>
      {keys.map((key) => (
        <kbd
          key={key}
          className="flex h-6 min-w-6 items-center justify-center rounded-md border border-b-2 bg-foreground/5 px-1.5 font-sans text-xs font-medium text-muted-foreground"
        >
          {key}
        </kbd>
      ))}
    </kbd>
  )
}
