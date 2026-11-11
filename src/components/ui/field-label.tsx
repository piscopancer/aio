import type { LucideIcon } from 'lucide-react'
import type { ComponentProps, ReactElement } from 'react'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

/** What opens and closes a piece of code in a hint. */
const CODE_MARK = '`'

/** What the label of a field may carry besides its text: the icon shown before it, and what the field is for, shown on hover, with `code` in backticks as in Markdown. */
type FieldLabelProps = { icon?: LucideIcon | undefined; hint?: string | undefined }

/** Title above a form field in the muted style every field shares, led by its icon when it has one, that explains the field on hover when given a hint. */
function FieldLabel({ icon: Icon, hint, className, children, ...props }: ComponentProps<'label'> & FieldLabelProps) {
  const label = (
    <label
      className={cn(
        'flex items-center gap-1.5 self-start text-sm font-semibold text-muted-foreground',
        hint !== undefined && 'cursor-help',
        className,
      )}
      {...props}
    >
      {Icon !== undefined && <Icon className="size-3.5 shrink-0" />}
      {children}
    </label>
  )
  if (hint === undefined) return label
  return <ExplainOnHover hint={hint}>{label}</ExplainOnHover>
}

/** Explains the name it wraps in a tooltip above it, with `code` in backticks as in Markdown. */
function ExplainOnHover({ hint, children }: { hint: string; children: ReactElement }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side="top" align="start" className="max-w-72 text-pretty">
        {hint.split(CODE_MARK).map((part, index) =>
          // every second part sits between a pair of backticks
          index % 2 === 1 ? (
            <code key={index} className="rounded bg-foreground/10 px-1 py-px font-mono text-[0.95em]">
              {part}
            </code>
          ) : (
            part
          ),
        )}
      </TooltipContent>
    </Tooltip>
  )
}

export { ExplainOnHover, FieldLabel, type FieldLabelProps }
