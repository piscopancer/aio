import { create } from 'zustand'
import type { Track } from '@/bindings'

/** The song whose details dialog is open; the dialog lives in the shell, so menus that open it may close at once. */
export const useTrackEditor = create<{ track: Track | null }>()(() => ({ track: null }))

export function editTrack(track: Track) {
  useTrackEditor.setState({ track })
}

export function closeTrackEditor() {
  useTrackEditor.setState({ track: null })
}
