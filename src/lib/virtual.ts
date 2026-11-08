import { useVirtualizer } from '@tanstack/react-virtual'
import { createContext, use, useLayoutEffect, useRef, useState } from 'react'

/** The scrolling main view, shared so long lists can virtualize against it. */
export const ScrollContainerContext = createContext<HTMLElement | null>(null)

/** Virtualizes a list that sits somewhere inside `scrollElement`, below content of any height. */
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
    overscan: 10,
    scrollMargin,
  })
  return { listRef, virtualizer }
}

/** Virtualizes a list inside the main view. */
export function useMainListVirtualizer(count: number, rowHeight: number) {
  return useListVirtualizer(use(ScrollContainerContext), count, rowHeight)
}
