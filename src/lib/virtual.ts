import { useVirtualizer } from '@tanstack/react-virtual'
import { createContext, use, useLayoutEffect, useRef, useState } from 'react'

/** The scrolling main view, shared so long lists can virtualize against it. */
export const ScrollContainerContext = createContext<HTMLElement | null>(null)

/** Space between the rows of a list, in pixels, so neighbouring highlighted rows never touch; `gap-1` in the lists that are not virtualized. */
export const ROW_GAP = 4

/** Virtualizes a list that sits somewhere inside `scrollElement`, below content of any height. It hands out the rows
 * and sizes as plain values rather than the virtualizer itself: the virtualizer is one object that changes inside, so
 * React Compiler, which memoizes on identity, would keep the rows a component first read from it forever. */
export function useListVirtualizer(scrollElement: HTMLElement | null, count: number, rowHeight: number) {
  const listRef = useRef<HTMLDivElement>(null)
  const [scrollMargin, setScrollMargin] = useState(0)

  useLayoutEffect(() => {
    const list = listRef.current
    if (scrollElement === null || list === null) return
    const measure = () =>
      setScrollMargin(
        list.getBoundingClientRect().top - scrollElement.getBoundingClientRect().top + scrollElement.scrollTop,
      )
    measure()
    // whatever sits above the list (headers, grids) resizes the content, which moves the list
    const observer = new ResizeObserver(measure)
    for (const child of scrollElement.children) observer.observe(child)
    return () => observer.disconnect()
  }, [scrollElement])

  const virtualizer = useVirtualizer({
    count,
    getScrollElement: () => scrollElement,
    estimateSize: () => rowHeight,
    gap: ROW_GAP,
    overscan: 10,
    // a scroll frame may render a moment later instead of forcing a synchronous React render inside the scroll event
    useFlushSync: false,
    scrollMargin,
  })
  return {
    listRef,
    items: virtualizer.getVirtualItems(),
    totalSize: virtualizer.getTotalSize(),
    scrollMargin,
    /** Scrolls the row at `index` to the middle of the scrolling view. */
    centerRow: (index: number) => virtualizer.scrollToIndex(index, { align: 'center' }),
  }
}

/** Virtualizes a list inside the main view. */
export function useMainListVirtualizer(count: number, rowHeight: number) {
  return useListVirtualizer(use(ScrollContainerContext), count, rowHeight)
}
