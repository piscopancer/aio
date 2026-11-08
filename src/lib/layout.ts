import { create } from 'zustand'
import { persist } from 'zustand/middleware'

/** Panels of the window layout the user can show, hide or shrink. */
const initialLayout = { queueOpen: false, sidebarCompact: false }

/** Only the sidebar mode is remembered across restarts; the queue always starts closed. */
export const useLayout = create<typeof initialLayout>()(
  persist(() => initialLayout, { name: 'aio-layout', partialize: ({ sidebarCompact }) => ({ sidebarCompact }) }),
)

export function toggleQueue() {
  useLayout.setState(({ queueOpen }) => ({ queueOpen: !queueOpen }))
}

/** Switches the playlists column between full rows and covers only. */
export function toggleSidebar() {
  useLayout.setState(({ sidebarCompact }) => ({ sidebarCompact: !sidebarCompact }))
}
