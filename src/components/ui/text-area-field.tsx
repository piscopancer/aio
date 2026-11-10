import { type ComponentProps, useId } from 'react'
import { FieldLabel, type FieldLabelProps } from '@/components/ui/field-label'
import { cn } from '@/lib/utils'

/** Text area with its label above it, styled like the text fields, for text that may run over several lines. */
function TextAreaField({
  label,
  icon,
  hint,
  className,
  ...props
}: ComponentProps<'textarea'> & FieldLabelProps & { label: string }) {
  const id = useId()
  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      <FieldLabel htmlFor={id} icon={icon} hint={hint}>
        {label}
      </FieldLabel>
      <textarea
        id={id}
        className="field-sizing-content min-h-20 w-full min-w-0 resize-none rounded-md border border-transparent bg-secondary px-3 py-2.5 text-sm outline-none select-text placeholder:text-subtle-foreground focus-visible:border-input focus-visible:bg-accent disabled:opacity-50"
        {...props}
      />
    </div>
  )
}

export { TextAreaField }
