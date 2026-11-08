import { type ComponentProps, type KeyboardEvent, type ReactNode, useEffect, useId, useRef, useState } from 'react'
import { ClearButton } from '@/components/ui/clear-button'
import { Input } from '@/components/ui/input'
import { Popover, PopoverAnchor, PopoverContent } from '@/components/ui/popover'
import { cn } from '@/lib/utils'

/** One suggestion: what it shows, and its key among the others. */
type Option = { key: string | number; label: string; subtitle?: string; art?: ReactNode }

/** Text field with its label above it that suggests existing values as you type; picking one fills the field. */
export function Combobox<const O extends Option>({
  label,
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
}: Omit<ComponentProps<typeof Input>, 'value' | 'onChange'> & {
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
  const [active, setActive] = useState(0)
  const anchorRef = useRef<HTMLDivElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const shown = open && options.length > 0

  useEffect(() => {
    listRef.current?.querySelector('[data-active=true]')?.scrollIntoView({ block: 'nearest' })
  }, [active])

  const pick = (option: O) => {
    onPick(option)
    setOpen(false)
  }

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    // the owner's keys come first, and a handled one is not handled again here
    onFieldKeyDown?.(event)
    if (event.defaultPrevented) return
    const step = { ArrowDown: 1, ArrowUp: -1 }[event.key]
    if (step !== undefined && options.length > 0) {
      event.preventDefault()
      setOpen(true)
      setActive((index) => (index + step + options.length) % options.length)
    } else if (event.key === 'Enter' && shown) {
      const option = options[active]
      if (option === undefined) return
      // the field picks the suggestion instead of submitting the form
      event.preventDefault()
      pick(option)
    } else if (event.key === 'Escape' && shown) {
      // closes the suggestions without closing the dialog around the field
      event.stopPropagation()
      setOpen(false)
    }
  }

  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      <label htmlFor={id} className="text-sm font-semibold text-muted-foreground">
        {label}
      </label>
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
                setActive(0)
                setOpen(true)
              }}
              onFocus={() => setOpen(true)}
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
