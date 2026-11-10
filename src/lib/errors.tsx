import { toast } from 'sonner'
import { create } from 'zustand'
import { type CommandCall, CommandError } from '@/lib/ipc'

/** What the details dialog shows of a failure: the readable message, the backend call it came from, the error as it arrived and where the frontend threw it. */
export type ErrorReport = {
  readonly message: string
  readonly call: CommandCall | null
  readonly raw: unknown
  readonly stack: string | null
}

/** The failure whose details are open, `null` while the dialog is closed. */
export const useErrorDetails = create<ErrorReport | null>()(() => null)

export const closeErrorDetails = () => useErrorDetails.setState(null, true)

function describe(error: unknown) {
  if (error instanceof CommandError)
    return { message: error.message, call: error.call, raw: error.error, stack: error.stack ?? null }
  if (error instanceof Error)
    return {
      message: error.message,
      call: null,
      raw: { name: error.name, message: error.message },
      stack: error.stack ?? null,
    }
  return { message: String(error), call: null, raw: error, stack: null }
}

/** Toasts `error`, after `context` when given; clicking the toast opens the details dialog with everything known about it. */
export function reportError(error: unknown, context?: string) {
  const described = describe(error)
  const report: ErrorReport = {
    ...described,
    message: context === undefined ? described.message : `${context}: ${described.message}`,
  }
  toast.error(
    // the button's overlay stretches over the whole toast, so a click anywhere on it opens the details
    <button
      type="button"
      onClick={() => useErrorDetails.setState(report, true)}
      className="text-left after:absolute after:inset-0 after:cursor-pointer"
    >
      {report.message}
    </button>,
  )
}
