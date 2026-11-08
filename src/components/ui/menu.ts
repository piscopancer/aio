/** Classes shared by the context menu and the dropdown menu, so both look the same. */
export const menuClasses = {
  content:
    'z-50 max-h-(--radix-popper-available-height) min-w-48 overflow-x-hidden overflow-y-auto rounded-md bg-popover p-1 text-popover-foreground shadow-xl shadow-black/50 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-in data-[state=open]:fade-in-0',
  item: "relative flex cursor-default items-center gap-3 rounded-sm px-3 py-2.5 text-sm outline-none select-none focus:bg-accent data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[variant=destructive]:text-destructive [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-muted-foreground",
  subTrigger:
    "flex cursor-default items-center gap-3 rounded-sm px-3 py-2.5 text-sm outline-none select-none focus:bg-accent data-[state=open]:bg-accent [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-muted-foreground",
  separator: '-mx-1 my-1 h-px bg-border',
} as const
