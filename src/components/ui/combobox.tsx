import { type ComponentProps, type KeyboardEvent, type ReactNode, useEffect, useId, useRef, useState } from 'react'
import { ClearButton } from '@/components/ui/clear-button'
import { FieldLabel, type FieldLabelProps } from '@/components/ui/field-label'
import { Input } from '@/components/ui/input'
import { Popover, PopoverAnchor, PopoverContent } from '@/components/ui/popover'
import { cn } from '@/lib/utils'

/** One suggestion: what it shows, and its key among the others. */
type Option = { key: string | number; label: string; subtitle?: string; art?: ReactNode }

/** The suggestion an arrow key moves to from `index`: from none to the first or the last, and past either end back to none, where the typed text stands. */
const stepped = (index: number | null, step: number, count: number) => {
  if (index === null) return step > 0 ? 0 : count - 1
  const next = index + step
  return next < 0 || next >= count ? null : next
}

/** Text field with its label above it that suggests existing values as you type; picking one fills the field, while Enter with none highlighted keeps the typed text. */
export function Combobox<const O extends Option>({
  label,
  icon,
  hint,
  value,
  onValueChange,
  options,
  onPick,
  art,
  className,
  onKeyDown: onFieldKeyDown,
  onBlur: onFieldBlur,
  children,
  ...props
}: Omit<ComponentProps<typeof Input>, 'value' | 'onChange'> &
  FieldLabelProps & {
    label: string
    value: string
    onValueChange: (value: string) => void
    /** Suggestions for the current text, best first. */
    options: readonly O[]
    onPick: (option: O) => void
    /** Artwork of the existing item the text names, shown inside the field at its start. */
    art?: ReactNode
    /** Shown under the field, such as the values already picked. */
    children?: ReactNode
  }) {
  const id = useId()
  const [open, setOpen] = useState(false)
  /** The highlighted suggestion; none at first, so Enter keeps what was typed. */
  const [active, setActive] = useState<number | null>(null)
  const anchorRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const shown = open && options.length > 0

  useEffect(() => {
    listRef.current?.querySelector('[data-active=true]')?.scrollIntoView({ block: 'nearest' })
  }, [active])

  const pick = (option: O) => {
    onPick(option)
    setOpen(false)
    // the next suggestions start unhighlighted, so a second Enter does not pick one of them
    setActive(null)
  }

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    const highlighted = shown && active !== null ? options[active] : undefined
    if (event.key === 'Enter' && highlighted !== undefined) {
      // the field picks the suggestion instead of submitting the form
      event.preventDefault()
      pick(highlighted)
      return
    }
    // the owner's keys come next, Enter among them for the typed text, and a handled one is not handled again here
    onFieldKeyDown?.(event)
    if (event.defaultPrevented) return
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key]
    if (step !== undefined && options.length > 0) {
      event.preventDefault()
      setOpen(true)
      setActive((index) => stepped(index, step, options.length))
    } else if (event.key === 'Enter' && shown) {
      // the typed text stands as it is, and the suggestions close instead of the form being submitted
      event.preventDefault()
      setOpen(false)
    } else if (event.key === 'Escape' && shown) {
      // closes the suggestions without closing the dialog around the field
      event.stopPropagation()
      setOpen(false)
    }
  }

  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      <FieldLabel htmlFor={id} icon={icon} hint={hint}>
        {label}
      </FieldLabel>
      <Popover open={shown} onOpenChange={setOpen}>
        <PopoverAnchor asChild>
          <div ref={anchorRef} className="relative flex items-center">
            {art !== undefined && <div className="pointer-events-none absolute left-1.5 size-6 *:size-full">{art}</div>}
            <Input
              id={id}
              role="combobox"
              aria-expanded={shown}
              autoComplete="off"
              spellCheck={false}
              value={value}
              onChange={(event) => {
                onValueChange(event.target.value)
                setActive(null)
                setOpen(true)
              }}
              onFocus={() => {
                setActive(null)
                setOpen(true)
              }}
              onBlur={(event) => {
                setOpen(false)
                onFieldBlur?.(event)
              }}
              onKeyDown={onKeyDown}
              className={cn(art !== undefined && 'pl-9', value !== '' && 'pr-9')}
              {...props}
            />
            {value !== '' && <ClearButton onClear={() => onValueChange('')} />}
          </div>
        </PopoverAnchor>
        <PopoverContent
          align="start"
          sideOffset={4}
          onOpenAutoFocus={(event) => event.preventDefault()}
          onInteractOutside={(event) => {
            if (event.target instanceof Node && anchorRef.current?.contains(event.target)) event.preventDefault()
          }}
          className="max-h-72 w-(--radix-popover-trigger-width) min-w-64 overflow-y-auto p-1"
        >
          <div ref={listRef} role="listbox" aria-label={label}>
            {options.map((option, index) => (
              <div
                key={option.key}
                role="option"
                aria-selected={index === active}
                data-active={index === active}
                onMouseMove={() => setActive(index)}
                // keeps the focus in the field, so picking does not blur it first
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => pick(option)}
                className="flex cursor-pointer items-center gap-3 rounded-md px-2 py-1.5 data-[active=true]:bg-foreground/10"
              >
                {option.art !== undefined && <div className="size-9 shrink-0 *:size-full">{option.art}</div>}
                <div className="flex min-w-0 flex-col">
                  <span className="truncate text-sm">{option.label}</span>
                  {option.subtitle !== undefined && (
                    <span className="truncate text-xs text-muted-foreground">{option.subtitle}</span>
                  )}
                </div>
              </div>
            ))}
          </div>
        </PopoverContent>
      </Popover>
      {children}
    </div>
  )
}
