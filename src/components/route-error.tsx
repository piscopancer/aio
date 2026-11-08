import { type ErrorComponentProps, Link, useRouter } from '@tanstack/react-router'
import { Button } from '@/components/ui/button'

/** Shown in place of a page whose data failed to load; the sidebar and the player keep working. */
export function RouteError({ error, reset }: ErrorComponentProps) {
  const router = useRouter()
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 px-6 text-center">
      <h1 className="text-3xl font-bold">This page failed to load</h1>
      <p className="max-w-lg text-muted-foreground select-text">
        {error instanceof Error ? error.message : String(error)}
      </p>
      <div className="flex gap-2">
        <Button
          variant="light"
          onClick={async () => {
            await router.invalidate()
            reset()
          }}
        >
          Try again
        </Button>
        <Button variant="outline" asChild>
          <Link to="/">Back to songs</Link>
        </Button>
      </div>
    </div>
  )
}
