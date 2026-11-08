import './styles.css'
import { QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider, createRouter } from '@tanstack/react-router'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { RouteError } from '@/components/route-error'
import { connectBackend } from '@/lib/backend'
import { queryClient } from '@/lib/query'
import { scheduleUpdateChecks } from '@/lib/updates'
import { routeTree } from '@/routeTree.gen'

const router = createRouter({
  routeTree,
  context: { queryClient },
  defaultPreload: 'intent',
  defaultErrorComponent: RouteError,
  // query data is cached and kept fresh by events, the router must not cache it a second time
  defaultPreloadStaleTime: 0,
})

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

const root = document.getElementById('root')
if (root === null) throw new Error('index.html has no #root element')

await connectBackend()
scheduleUpdateChecks()

createRoot(root).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>,
)
