import { useQuery } from '@tanstack/react-query'
import { Link, useMatchRoute } from '@tanstack/react-router'
import { Disc3Icon, FolderOpenIcon, MicVocalIcon, Music2Icon } from 'lucide-react'
import type { ReactNode } from 'react'
import { queries } from '@/lib/query'
import { cn } from '@/lib/utils'

/** Look of the chip of the current page: a lighter grey than the track it sits on. */
const activeTabClass = 'bg-foreground/15 text-foreground'

/** A chip of the tab bar; TanStack Router marks the one of the current page with `data-status=active`, styled like `activeTabClass` (spelled out, since Tailwind only sees literal class names). */
const tabClass =
  'flex h-8 items-center gap-2 rounded-full px-3.5 text-sm font-semibold text-muted-foreground transition-colors hover:text-foreground data-[status=active]:bg-foreground/15 data-[status=active]:text-foreground [&_svg]:size-4'

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

  return (
    <nav className="flex shrink-0 items-center gap-3 px-4 py-3">
      <TabGroup>
        <Link to="/settings/folders" className={cn(tabClass, inFolder && activeTabClass)}>
          <FolderOpenIcon />
          Folders
          <CountPill count={counts.folders} />
        </Link>
      </TabGroup>
      <TabGroup>
        <Link to="/artists" className={tabClass}>
          <MicVocalIcon />
          Artists
          <CountPill count={counts.artists} />
        </Link>
        <Link to="/albums" className={tabClass}>
          <Disc3Icon />
          Albums
          <CountPill count={counts.albums} />
        </Link>
        <Link to="/" className={tabClass} activeOptions={{ exact: true }}>
          <Music2Icon />
          Songs
          <CountPill count={counts.songs} />
        </Link>
      </TabGroup>
    </nav>
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
