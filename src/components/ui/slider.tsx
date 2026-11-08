import { Slider as SliderPrimitive } from 'radix-ui'
import type { ComponentProps } from 'react'
import { cn } from '@/lib/utils'

/** Thin player slider: the range turns accent and the thumb appears on hover. */
function Slider({ className, ...props }: ComponentProps<typeof SliderPrimitive.Root>) {
  return (
    <SliderPrimitive.Root
      data-slot="slider"
      className={cn(
        'group relative flex h-4 w-full touch-none items-center select-none data-[disabled]:opacity-50',
        className,
      )}
      {...props}
    >
      <SliderPrimitive.Track className="relative h-1 grow overflow-hidden rounded-full bg-foreground/30">
        <SliderPrimitive.Range className="absolute h-full rounded-full bg-foreground group-hover:bg-primary" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        aria-label={props['aria-label']}
        className="block size-3 rounded-full bg-foreground opacity-0 shadow-md outline-none group-hover:opacity-100 focus-visible:opacity-100"
      />
    </SliderPrimitive.Root>
  )
}

export { Slider }
