import { type ComponentProps, useId } from 'react'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

/** Input with its label above it. */
function TextField({ label, className, ...props }: ComponentProps<typeof Input> & { label: string }) {
  const id = useId()
  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      <label htmlFor={id} className="text-sm font-semibold text-muted-foreground">
        {label}
      </label>
      <Input id={id} {...props} />
    </div>
  )
}

export { TextField }
