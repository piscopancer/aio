import { useQuery } from '@tanstack/react-query'
import { Link, useMatchRoute } from '@tanstack/react-router'
import { Disc3Icon, FolderOpenIcon, MicVocalIcon, Music2Icon } from 'lucide-react'
import { motion } from 'motion/react'
import type { ReactNode } from 'react'
import type { PlaybackContext } from '@/bindings'
import { usePlayer } from '@/lib/player'
import { queries } from '@/lib/query'
import { cn } from '@/lib/utils'

/** A chip of the tab bar; TanStack Router marks the one of the current page with `data-status=active`. A chip is a link,
 * which the webview would let the user drag away as a URL, so dragging is off. It isolates its layers, so the grey
 * background gliding under its label still sits above the group's track. */
const tabClass =
  'relative isolate flex h-8 items-center gap-2 rounded-full px-3.5 text-sm font-semibold text-muted-foreground transition-colors [-webkit-user-drag:none] hover:text-foreground data-[status=active]:text-foreground [&_svg]:size-4'

/** The tab of the section the queue was started from takes the accent, like a playlist playing in the sidebar; it wins
 * over the colours of hovering and of the current page. */
const playingTabClass = 'text-primary!'

/** How the grey background of the current tab glides to the next one. */
const chipTransition = { type: 'spring', bounce: 0.15, duration: 0.25 } as const

/** Tabs of the library above the main view: Folders on its own, then Artists, Albums and Songs from general to specific. */
export function LibraryTabs() {
  const matchRoute = useMatchRoute()
  const counts = {
    songs: useQuery({ ...queries.tracks(), select: (list) => list.length }).data,
    albums: useQuery({ ...queries.albums(), select: (list) => list.length }).data,
    artists: useQuery({ ...queries.artists(), select: (list) => list.length }).data,
    folders: useQuery({ ...queries.folders(), select: (list) => list.length }).data,
  }
  // a folder's own page lives outside the settings route, yet belongs to the Folders tab
  const inFolder = matchRoute({ to: '/folders/$folderId', fuzzy: true }) !== false
  // playlists have no tab, the sidebar marks the one playing
  const playingKind = usePlayer((state) => state.context?.kind)
  const playingFrom = (kind: PlaybackContext['kind']) => playingKind === kind && playingTabClass

  return (
    <nav className="flex shrink-0 items-center gap-3 px-4 py-3">
      <TabGroup>
        <Link to="/settings/folders" className={cn(tabClass, inFolder && 'text-foreground', playingFrom('folder'))}>
          {({ isActive }) => (
            <TabLabel active={isActive || inFolder} icon={<FolderOpenIcon />} label="Folders" count={counts.folders} />
          )}
        </Link>
      </TabGroup>
      <TabGroup>
        <Link to="/artists" className={cn(tabClass, playingFrom('artist'))}>
          {({ isActive }) => (
            <TabLabel active={isActive} icon={<MicVocalIcon />} label="Artists" count={counts.artists} />
          )}
        </Link>
        <Link to="/albums" className={cn(tabClass, playingFrom('album'))}>
          {({ isActive }) => <TabLabel active={isActive} icon={<Disc3Icon />} label="Albums" count={counts.albums} />}
        </Link>
        <Link to="/" className={cn(tabClass, playingFrom('songs'))} activeOptions={{ exact: true }}>
          {({ isActive }) => <TabLabel active={isActive} icon={<Music2Icon />} label="Songs" count={counts.songs} />}
        </Link>
      </TabGroup>
    </nav>
  )
}

/** What a chip shows; the current one has the grey background, one element under a shared layout id, so it glides from
 * the old tab to the new one, across the groups too. */
function TabLabel({
  active,
  icon,
  label,
  count,
}: {
  active: boolean
  icon: ReactNode
  label: string
  count: number | undefined
}) {
  return (
    <>
      {active && (
        <motion.span
          layoutId="library-tab-chip"
          transition={chipTransition}
          className="absolute inset-0 -z-10 rounded-full bg-foreground/15"
        />
      )}
      {icon}
      {label}
      <CountPill count={count} />
    </>
  )
}

/** Chips that belong together, on one shared track. */
function TabGroup({ children }: { children: ReactNode }) {
  return <div className="flex items-center gap-0.5 rounded-full bg-foreground/5 p-1">{children}</div>
}

/** How many items a library section holds, as a small muted pill; hidden until loaded. */
function CountPill({ count }: { count: number | undefined }) {
  if (count === undefined) return null
  return (
    <span className="rounded-full bg-current/10 px-1.5 text-xs font-medium tabular-nums opacity-70">
      {count.toLocaleString()}
    </span>
  )
}
