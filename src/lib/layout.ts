import { create } from 'zustand'
import { persist } from 'zustand/middleware'

/** Panels that can take the right side of the window, one at a time. */
export type SidePanel = 'queue' | 'lyrics'

/** How the open lyrics are shown: in their panel, over the library and panel area, or over the whole screen. */
export type LyricsView = 'panel' | 'expanded' | 'fullscreen'

/** Orders the sidebar lists playlists in: the most recently played, the newest, or the most songs first. */
export const PLAYLIST_ORDERS = ['played', 'created', 'size'] as const
export type PlaylistOrder = (typeof PLAYLIST_ORDERS)[number]

/** Panels of the window layout the user can show, hide or shrink. */
const initialLayout: {
  panel: SidePanel | null
  lyricsView: LyricsView
  sidebarCompact: boolean
  playlistOrder: PlaylistOrder
} = {
  panel: null,
  lyricsView: 'panel',
  sidebarCompact: false,
  playlistOrder: 'played',
}

/** Only the sidebar mode and its playlist order are remembered across restarts; the side panels always start closed. */
export const useLayout = create<typeof initialLayout>()(
  persist(() => initialLayout, {
    name: 'aio-layout',
    partialize: ({ sidebarCompact, playlistOrder }) => ({ sidebarCompact, playlistOrder }),
  }),
)

export function setPlaylistOrder(playlistOrder: PlaylistOrder) {
  useLayout.setState({ playlistOrder })
}

/** Opens `panel` in place of the other one, or closes it when it is open. */
export function togglePanel(panel: SidePanel) {
  // lyrics always open in their panel, whatever view they were closed in
  useLayout.setState((state) => ({ panel: state.panel === panel ? null : panel, lyricsView: 'panel' }))
}

export function setLyricsView(lyricsView: LyricsView) {
  useLayout.setState({ lyricsView })
}

/** Switches the playlists column between full rows and covers only. */
export function toggleSidebar() {
  useLayout.setState(({ sidebarCompact }) => ({ sidebarCompact: !sidebarCompact }))
}

/** Keys that shrink and widen the playlists column. */
export const SIDEBAR_KEYS = ['Ctrl', 'B'] as const

/** Keys that go back and forward through the pages, like in a browser. */
export const BACK_KEYS = ['Alt', '←'] as const
export const FORWARD_KEYS = ['Alt', '→'] as const

/** Keys that open and close the lyrics panel. */
export const LYRICS_KEYS = ['Ctrl', 'L'] as const

/** Keys of the player's transport, shown in the hints of its buttons. */
export const PREVIOUS_KEYS = ['Ctrl', '←'] as const
export const NEXT_KEYS = ['Ctrl', '→'] as const
export const REPEAT_KEYS = ['Ctrl', 'R'] as const
export const MUTE_KEYS = ['Ctrl', 'M'] as const
