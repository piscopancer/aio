import { type ComponentProps, useId } from 'react'
import { FieldLabel, type FieldLabelProps } from '@/components/ui/field-label'
import { ClearButton } from '@/components/ui/clear-button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

/** Input with its label above it, and a cross that empties it when `onClear` is given. */
function TextField({
  label,
  icon,
  hint,
  className,
  onClear,
  ...props
}: ComponentProps<typeof Input> & FieldLabelProps & { label: string; onClear?: () => void }) {
  const id = useId()
  const clearable = onClear !== undefined && props.value !== '' && props.disabled !== true
  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      <FieldLabel htmlFor={id} icon={icon} hint={hint}>
        {label}
      </FieldLabel>
      <div className="relative flex items-center">
        <Input id={id} className={cn(clearable && 'pr-9')} {...props} />
        {clearable && <ClearButton onClear={onClear} />}
      </div>
    </div>
  )
}

export { TextField }
