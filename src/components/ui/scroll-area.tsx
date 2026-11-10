import { ScrollArea as ScrollAreaPrimitive } from 'radix-ui'
import type { ComponentProps, Ref } from 'react'
import { cn } from '@/lib/utils'

/** Scrolling area whose vertical bar floats over the content instead of taking a column of its own: it fades in while the area scrolls and out shortly after, so content that starts or stops overflowing never shifts sideways. `viewportRef` gets the element that actually scrolls. */
function ScrollArea({
  className,
  children,
  viewportRef,
  hideScrollbar = false,
  ...props
}: Omit<ComponentProps<typeof ScrollAreaPrimitive.Root>, 'type'> & {
  viewportRef?: Ref<HTMLDivElement> | undefined
  /** Keeps the bar hidden even while the area scrolls, for scrolling the app does by itself. */
  hideScrollbar?: boolean | undefined
}) {
  return (
    <ScrollAreaPrimitive.Root
      data-slot="scroll-area"
      type="scroll"
      className={cn('relative isolate overflow-hidden', className)}
      {...props}
    >
      {/* the content wrapper is a table by default, which lets wide content stretch the area instead of being held to its width */}
      <ScrollAreaPrimitive.Viewport ref={viewportRef} className="size-full rounded-[inherit] [&>div]:block!">
        {children}
      </ScrollAreaPrimitive.Viewport>
      <ScrollAreaPrimitive.Scrollbar
        data-slot="scroll-area-scrollbar"
        orientation="vertical"
        // kept mounted while hidden, so it can fade out rather than vanish
        forceMount
        className={cn(
          'z-20 flex w-2 touch-none p-0.5 transition-opacity duration-300 select-none data-[state=hidden]:pointer-events-none data-[state=hidden]:opacity-0',
          hideScrollbar && 'pointer-events-none opacity-0',
        )}
      >
        <ScrollAreaPrimitive.Thumb className="relative flex-1 rounded-full bg-foreground/25 transition-colors hover:bg-foreground/40 active:bg-foreground/50" />
      </ScrollAreaPrimitive.Scrollbar>
    </ScrollAreaPrimitive.Root>
  )
}

export { ScrollArea }
