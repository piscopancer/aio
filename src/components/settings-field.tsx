import { type ComponentProps, type ReactNode, useId } from 'react'
import { ExplainOnHover } from '@/components/ui/field-label'
import { Switch } from '@/components/ui/switch'
import { cn } from '@/lib/utils'

/** Name of a setting, which explains what the setting does when hovered. */
function SettingName({ hint, className, ...props }: { hint: string } & ComponentProps<'label'>) {
  return (
    <ExplainOnHover hint={hint}>
      <label className={cn('w-fit cursor-help', className)} {...props} />
    </ExplainOnHover>
  )
}

/** One setting of a settings tab: its name, explained on hover, and its controls below. */
export function Field({ title, hint, children }: { title: string; hint: string; children: ReactNode }) {
  return (
    <section className="grid gap-3">
      <SettingName hint={hint}>{title}</SettingName>
      {children}
    </section>
  )
}

/** A setting with one compact control, such as a slider: its name, explained on hover, and the control at the far end
 * of the row, or below the name when the tab is too narrow for both side by side. */
export function RowField({ title, hint, children }: { title: string; hint: string; children: ReactNode }) {
  return (
    <section className="@container">
      <div className="flex flex-col gap-3 @lg:flex-row @lg:items-center @lg:justify-between @lg:gap-6">
        <SettingName hint={hint} className="shrink-0">
          {title}
        </SettingName>
        <div className="w-full max-w-xs min-w-0">{children}</div>
      </div>
    </section>
  )
}

/** A setting that is on or off: its name, explained on hover, and the switch at the far end of the row. */
export function SwitchField({
  title,
  hint,
  ...props
}: { title: string; hint: string } & Pick<ComponentProps<typeof Switch>, 'checked' | 'disabled' | 'onCheckedChange'>) {
  const id = useId()
  return (
    <section className="flex items-center justify-between gap-4 has-disabled:opacity-50">
      <SettingName hint={hint} htmlFor={id}>
        {title}
      </SettingName>
      {/* the faded row already shows the switch is off limits */}
      <Switch id={id} className="disabled:opacity-100" {...props} />
    </section>
  )
}
