import { Link } from '@tanstack/react-router'
import type { ReactNode } from 'react'
import { Button } from '@/components/ui/button'

export function EmptyState({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="flex flex-col items-center gap-3 px-6 py-16 text-center">
      <h2 className="text-2xl font-bold">{title}</h2>
      <div className="max-w-md text-muted-foreground">{children}</div>
    </div>
  )
}

/** Shown by every library view until a folder with music is added. */
export function EmptyLibrary() {
  return (
    <EmptyState title="Your library is empty">
      <p>Add a folder with music and Aio will read every song in it.</p>
      <Button variant="light" className="mt-5" asChild>
        <Link to="/settings/folders">Add a folder</Link>
      </Button>
    </EmptyState>
  )
}
