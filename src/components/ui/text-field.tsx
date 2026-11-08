import { type ComponentProps, useId } from 'react'
import { ClearButton } from '@/components/ui/clear-button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

/** Input with its label above it, and a cross that empties it when `onClear` is given. */
function TextField({
  label,
  className,
  onClear,
  ...props
}: ComponentProps<typeof Input> & { label: string; onClear?: () => void }) {
  const id = useId()
  const clearable = onClear !== undefined && props.value !== '' && props.disabled !== true
  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      <label htmlFor={id} className="text-sm font-semibold text-muted-foreground">
        {label}
      </label>
      <div className="relative flex items-center">
        <Input id={id} className={cn(clearable && 'pr-9')} {...props} />
        {clearable && <ClearButton onClear={onClear} />}
      </div>
    </div>
  )
}

export { TextField }
