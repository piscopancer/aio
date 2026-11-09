import { useMutation, useQuery } from '@tanstack/react-query'
import { type DragEvent, useState } from 'react'
import type { PinTarget } from '@/bindings'
import { carriesPin, startPinDrag } from '@/lib/dnd'
import { mutations, queries } from '@/lib/query'

type PinKind = PinTarget['kind']

/** An item of a list that can lead it when pinned. */
type Pinnable = { id: number; pin: number | null }

/** The pinned items in their order, and the rest as given. */
export function splitPinned<const T extends Pinnable>(items: readonly T[]) {
  return {
    pinned: items.filter((item) => item.pin !== null).toSorted((a, b) => (a.pin ?? 0) - (b.pin ?? 0)),
    rest: items.filter((item) => item.pin === null),
  }
}

/** The target of an item of a pin list, by the list's kind. */
export function pinTarget(kind: PinKind, id: number): PinTarget {
  switch (kind) {
    case 'playlist':
      return { kind, id }
    case 'album':
      return { kind, id }
    case 'artist':
      return { kind, id }
    default:
      return kind satisfies never
  }
}

/** Whether `target` is pinned, read from the cached list it is pinned in. */
export function usePinned(target: PinTarget) {
  const lists = {
    playlist: useQuery({ ...queries.playlists(), enabled: target.kind === 'playlist' }).data,
    album: useQuery({ ...queries.albums(), enabled: target.kind === 'album' }).data,
    artist: useQuery({ ...queries.artists(), enabled: target.kind === 'artist' }).data,
  } satisfies Record<PinKind, readonly Pinnable[] | undefined>
  return lists[target.kind]?.some((item) => item.id === target.id && item.pin !== null) ?? false
}

/** Pins `target` when it is not pinned, unpins it otherwise. */
export function usePinToggle(target: PinTarget) {
  const pinned = usePinned(target)
  const { mutate: pin } = useMutation(mutations.pin())
  const { mutate: unpin } = useMutation(mutations.unpin())
  return { pinned, toggle: () => (pinned ? unpin(target) : pin(target)) }
}

/** How the pinned items of a list sit: one under another, or side by side in a grid. */
type PinAxis = 'list' | 'grid'

/** Common part of the drop line: a rounded accent bar drawn only while the item is the drop target. */
const PIN_DROP_LINE =
  'relative before:pointer-events-none before:absolute before:hidden before:rounded-full before:bg-primary data-[pin-drop]:before:block'

/** Line in the middle of the 4px gap where the dragged pin would land, so after one item and before the next draw the same line; pinned grid cards drop content-visibility, which would clip it. */
export const pinDropMarks = {
  list: `${PIN_DROP_LINE} before:inset-x-2 before:h-0.5 data-[pin-drop=after]:before:-bottom-[3px] data-[pin-drop=before]:before:-top-[3px]`,
  grid: `${PIN_DROP_LINE} before:inset-y-3 before:w-0.5 [content-visibility:visible] data-[pin-drop=after]:before:-right-[3px] data-[pin-drop=before]:before:-left-[3px]`,
} as const satisfies Record<PinAxis, string>

/** Set on the root element while a pinned item is dragged, which the `pin-dragging` variant of the styles reads. */
const PIN_DRAG_ATTRIBUTE = 'data-pin-dragging'

/** Classes of an item that is not pinned: while a pinned one is dragged it fades and ignores the pointer, as it
 * takes no part in the reordering. */
export const PIN_IDLE = 'transition pin-dragging:pointer-events-none pin-dragging:opacity-40'

type DropEdge = 'before' | 'after'

/** Drag-and-drop ordering of the pinned items `ids` of a list; spread `dragProps(id)` on each pinned item. */
export function usePinReorder(kind: PinKind, ids: readonly number[], axis: PinAxis) {
  const { mutate: move } = useMutation(mutations.movePin())
  const [dragged, setDragged] = useState<number | null>(null)
  const [drop, setDrop] = useState<{ id: number; edge: DropEdge } | null>(null)
  const end = () => {
    setDragged(null)
    setDrop(null)
    document.documentElement.removeAttribute(PIN_DRAG_ATTRIBUTE)
  }

  return (id: number) => ({
    draggable: true,
    'data-pin-drop': drop?.id === id ? drop.edge : undefined,
    onDragStart: (event: DragEvent<HTMLElement>) => {
      startPinDrag(event)
      setDragged(id)
      document.documentElement.setAttribute(PIN_DRAG_ATTRIBUTE, '')
    },
    onDragOver: (event: DragEvent<HTMLElement>) => {
      if (dragged === null || !carriesPin(event)) return
      event.preventDefault()
      event.dataTransfer.dropEffect = 'move'
      const { top, left, width, height } = event.currentTarget.getBoundingClientRect()
      const before = axis === 'list' ? event.clientY < top + height / 2 : event.clientX < left + width / 2
      const edge = before ? 'before' : 'after'
      if (drop?.id !== id || drop.edge !== edge) setDrop({ id, edge })
    },
    onDrop: (event: DragEvent<HTMLElement>) => {
      if (!carriesPin(event)) return
      event.preventDefault()
      const from = dragged === null ? -1 : ids.indexOf(dragged)
      const at = ids.indexOf(id)
      const edge = drop?.edge
      end()
      if (dragged === null || from < 0 || at < 0 || edge === undefined) return
      const insert = edge === 'before' ? at : at + 1
      // the backend inserts into the order that remains after taking the pin out
      const to = insert > from ? insert - 1 : insert
      if (to !== from) move({ target: pinTarget(kind, dragged), to })
    },
    onDragEnd: end,
  })
}
