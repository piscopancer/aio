import { relaunch } from '@tauri-apps/plugin-process'
import { type Update, check } from '@tauri-apps/plugin-updater'
import { create } from 'zustand'

/** Where the app stands with updates; a found update downloads right away and waits for a restart. */
export type UpdateStatus =
  | { kind: 'idle' }
  | { kind: 'checking' }
  | { kind: 'latest' }
  | { kind: 'downloading'; version: string; percent: number }
  | { kind: 'ready'; version: string }
  | { kind: 'failed'; message: string }
  | { kind: 'unsupported' }

const FIRST_CHECK_DELAY_MS = 10_000
const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000
/** A dev build has no signed release to update from. */
const supported = !import.meta.env.DEV

export const useUpdates = create<UpdateStatus>()(() => (supported ? { kind: 'idle' } : { kind: 'unsupported' }))

/** The downloaded update, installed on the next `installUpdate`. */
let downloaded: Update | null = null

const busyKinds = ['checking', 'downloading', 'ready'] as const satisfies UpdateStatus['kind'][]

/** Asks the latest published release for a newer version and downloads it; does nothing while one is under way. */
export async function checkForUpdates() {
  const { kind } = useUpdates.getState()
  if (!supported || busyKinds.some((busy) => busy === kind)) return
  useUpdates.setState({ kind: 'checking' }, true)
  try {
    const update = await check()
    if (update === null) return useUpdates.setState({ kind: 'latest' }, true)
    let total = 0
    let received = 0
    const progress = (percent: number) =>
      useUpdates.setState({ kind: 'downloading', version: update.version, percent }, true)
    progress(0)
    await update.download((event) => {
      if (event.event === 'Started') total = event.data.contentLength ?? 0
      if (event.event === 'Progress' && total > 0) progress(((received += event.data.chunkLength) / total) * 100)
    })
    downloaded = update
    useUpdates.setState({ kind: 'ready', version: update.version }, true)
  } catch (error) {
    useUpdates.setState({ kind: 'failed', message: String(error) }, true)
  }
}

/** Runs the downloaded installer, which closes the app, and starts the new version. */
export async function installUpdate() {
  if (downloaded === null) return
  try {
    await downloaded.install()
    await relaunch()
  } catch (error) {
    useUpdates.setState({ kind: 'failed', message: String(error) }, true)
  }
}

/** Checks shortly after launch, so startup stays quick, and then every few hours. */
export function scheduleUpdateChecks() {
  if (!supported) return
  setTimeout(() => {
    void checkForUpdates()
    setInterval(() => void checkForUpdates(), CHECK_INTERVAL_MS)
  }, FIRST_CHECK_DELAY_MS)
}
