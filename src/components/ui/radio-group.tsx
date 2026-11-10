import { RadioGroup as RadioGroupPrimitive } from 'radix-ui'
import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

function RadioGroup({ className, ...props }: ComponentProps<typeof RadioGroupPrimitive.Root>) {
  return <RadioGroupPrimitive.Root data-slot="radio-group" className={cn('grid gap-2', className)} {...props} />
}

/** A whole row that selects its option, highlighted while selected. */
function RadioCard({ className, ...props }: ComponentProps<typeof RadioGroupPrimitive.Item>) {
  return (
    <RadioGroupPrimitive.Item
      data-slot="radio-card"
      className={cn(
        'flex cursor-pointer items-center gap-3 rounded-md border border-border px-3 py-2 text-left text-sm transition-colors outline-none hover:bg-surface-hover focus-visible:ring-1 focus-visible:ring-ring/50 data-[state=checked]:border-primary data-[state=checked]:bg-primary/10',
        className,
      )}
      {...props}
    />
  )
}

export { RadioCard, RadioGroup }
