import type { DragEvent } from 'react'
import type { TrackId } from '@/bindings'

/** Drag payload type of a library track, accepted by playlists in the sidebar and by playlist rows. */
const TRACK_MIME = 'application/x-aio-track'

export function startTrackDrag(event: DragEvent, id: TrackId) {
  event.dataTransfer.setData(TRACK_MIME, String(id))
  event.dataTransfer.effectAllowed = 'copyMove'
}

/** Whether the drag carries a track; the payload itself is readable only on drop. */
export function carriesTrack(event: DragEvent) {
  return event.dataTransfer.types.includes(TRACK_MIME)
}

/** Drag payload type of a pinned item being moved among the pinned of its list; the list itself knows which one. */
const PIN_MIME = 'application/x-aio-pin'

export function startPinDrag(event: DragEvent) {
  event.dataTransfer.setData(PIN_MIME, '')
  event.dataTransfer.effectAllowed = 'move'
}

export function carriesPin(event: DragEvent) {
  return event.dataTransfer.types.includes(PIN_MIME)
}

export function droppedTrack(event: DragEvent) {
  const id = Number(event.dataTransfer.getData(TRACK_MIME))
  return Number.isSafeInteger(id) && id > 0 ? id : undefined
}
