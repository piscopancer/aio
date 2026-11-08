import type { ComponentProps, ReactNode } from 'react'
import { Shortcut } from '@/components/ui/shortcut'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

/** Tooltip around a single control, with the keys that do the same when given. */
export function Hint({
  label,
  side,
  keys,
  children,
}: {
  label: string
  side?: ComponentProps<typeof TooltipContent>['side']
  keys?: readonly string[]
  children: ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent {...(side && { side })} className={keys && 'flex items-center gap-2'}>
        {label}
        {keys && <Shortcut keys={keys} />}
      </TooltipContent>
    </Tooltip>
  )
}
