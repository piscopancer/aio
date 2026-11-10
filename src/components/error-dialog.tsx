import type { ReactNode } from 'react'
import { Dialog, DialogBody, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { closeErrorDetails, useErrorDetails } from '@/lib/errors'

/** Everything known about the failure whose toast was clicked: the backend call with its arguments, the error as it arrived and the frontend stack. */
export function ErrorDialog() {
  const report = useErrorDetails()
  return (
    <Dialog open={report !== null} onOpenChange={(open) => !open && closeErrorDetails()}>
      {report !== null && (
        <DialogContent className="max-w-3xl" aria-describedby={undefined}>
          <DialogHeader>
            <DialogTitle>{report.message}</DialogTitle>
          </DialogHeader>
          <DialogBody>
            {report.call !== null && (
              <Section title="Command">{`${report.call.command}(${report.call.args.map((arg) => JSON.stringify(arg)).join(', ')})`}</Section>
            )}
            <Section title="Error">{JSON.stringify(report.raw, null, 2) ?? String(report.raw)}</Section>
            {report.stack !== null && <Section title="Stack">{report.stack}</Section>}
          </DialogBody>
        </DialogContent>
      )}
    </Dialog>
  )
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-sm font-medium text-muted-foreground">{title}</h3>
      <pre className="overflow-x-auto rounded-md bg-surface p-3 font-mono text-xs leading-relaxed whitespace-pre select-text">
        {children}
      </pre>
    </section>
  )
}
