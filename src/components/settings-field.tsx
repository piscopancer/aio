import type { ReactNode } from 'react'

/** One setting of a settings tab: its title, a line on what it does, and its controls below. */
export function Field({ title, hint, children }: { title: string; hint: string; children: ReactNode }) {
  return (
    <section className="grid gap-3">
      <div className="grid gap-1">
        <h4 className="font-semibold">{title}</h4>
        <p className="text-sm text-muted-foreground">{hint}</p>
      </div>
      {children}
    </section>
  )
}
