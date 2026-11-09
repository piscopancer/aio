import { CatchBoundary, Outlet, useRouter, useRouterState } from '@tanstack/react-router'
import { Activity, Suspense, use, useEffect, useLayoutEffect, useRef } from 'react'
import { RouteError } from '@/components/route-error'
import { ScrollContainerContext } from '@/lib/virtual'
import { Route as albumsRoute } from '@/routes/albums/index'
import { Route as artistsRoute } from '@/routes/artists/index'
import { Route as songsRoute } from '@/routes/index'
import { Route as foldersRoute } from '@/routes/settings/folders'

/** Pages of the library tabs, which are heavy to build; they are all rendered from the start and only hidden while
 * another page is shown, so a tab switch shows its page at once, scrolled where it was left. */
const tabRoutes = [foldersRoute, artistsRoute, albumsRoute, songsRoute] as const

type TabRouteId = (typeof tabRoutes)[number]['id']

// the pages' code is split from the route files; loaded up front, they render at once instead of suspending outside
// the router, which only knows how to wait for them inside its own outlet
await Promise.all(tabRoutes.map((route) => route.options.component?.preload?.()))

/** Each page as one element made once: switching tabs hands every `Activity` the very same children, so React skips
 * the pages entirely, the hidden ones included, instead of rebuilding hundreds of cards and rows on every switch. */
const tabPages = tabRoutes.map((route) => {
  const Page = route.options.component
  return {
    // the route's id is read at render, since the route tree assigns it only after this module has run
    route,
    page: (
      <CatchBoundary getResetKey={() => route.id} errorComponent={RouteError}>
        <Suspense>{Page && <Page />}</Suspense>
      </CatchBoundary>
    ),
  }
})

/** The page of the current route inside the scrolling main view: a kept tab page, or else the router's outlet. */
export function MainView() {
  const router = useRouter()
  const scrollElement = use(ScrollContainerContext)
  const routeId = useRouterState({ select: (state) => state.matches.at(-1)?.routeId })
  const pathname = useRouterState({ select: (state) => state.location.pathname })
  const shownTab = tabRoutes.find((route) => route.id === routeId)?.id
  const tabScroll = useRef(new Map<TabRouteId, number>())

  // noted before the navigation hides the tab, since a hidden page has no height left to keep its scroll
  useEffect(
    () =>
      router.subscribe('onBeforeNavigate', () => {
        if (shownTab !== undefined && scrollElement !== null) {
          tabScroll.current.set(shownTab, scrollElement.scrollTop)
        }
      }),
    [router, scrollElement, shownTab],
  )

  // before paint, so a tab never flashes at the place of the page shown before it
  useLayoutEffect(() => {
    const top = shownTab === undefined ? 0 : (tabScroll.current.get(shownTab) ?? 0)
    scrollElement?.scrollTo({ top })
  }, [scrollElement, pathname, shownTab])

  return (
    <>
      {tabPages.map(({ route, page }) => (
        <Activity key={route.id} mode={route.id === shownTab ? 'visible' : 'hidden'}>
          {page}
        </Activity>
      ))}
      {shownTab === undefined && <Outlet />}
    </>
  )
}
