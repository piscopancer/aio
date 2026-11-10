import { replaceEqualDeep } from '@tanstack/react-query'
import { toast } from 'sonner'
import { create } from 'zustand'
import {
  type PlaybackContext,
  type PlayerState,
  type QueueKey,
  type RepeatMode,
  type TrackId,
  events,
} from '@/bindings'
import { reportError } from '@/lib/errors'
import { plural } from '@/lib/format'
import { commands, unwrap } from '@/lib/ipc'
import { invalidatePlaylists } from '@/lib/query'

/** The whole library in its Songs order, the context of songs played from outside any page. */
export const songsContext = { kind: 'songs' } as const satisfies PlaybackContext

const initialState = await commands.getPlayerState()

/** Mirror of the backend player; Rust owns the state and pushes every change. */
export const usePlayer = create<PlayerState>()(() => initialState)

/** Takes a full state from the backend, keeping the objects of the queue, its tracks and the context that did not change, so a pause, a volume step or a shuffle does not hand every reader of the playing track a new copy of it; queue items are matched by key, as a shuffle moves them. */
function receive(next: PlayerState) {
  usePlayer.setState((state) => {
    const before = new Map(state.queue.map((item) => [item.key, item]))
    const queue = next.queue.map((item) => replaceEqualDeep(before.get(item.key), item))
    return replaceEqualDeep(state, { ...next, queue })
  })
}

/** Subscribes the mirror to backend events, then re-reads the state to cover changes made before the subscription. */
export async function syncPlayer() {
  await Promise.all([
    events.playerStateChanged.listen(({ payload }) => receive(payload)),
    events.playbackProgress.listen(({ payload }) => usePlayer.setState(payload)),
  ])
  receive(await commands.getPlayerState())
}

export function currentItem(state: PlayerState) {
  return state.index === null ? undefined : state.queue[state.index]
}

export function sameContext(a: PlaybackContext, b: PlaybackContext) {
  return a.kind === b.kind && (a.kind === 'songs' || (b.kind !== 'songs' && a.id === b.id))
}

/** Whether `context` is the one the queue was started from. */
export function useIsPlayingFrom(context: PlaybackContext) {
  return usePlayer((state) => state.context !== null && sameContext(state.context, context))
}

/** Same kind and id means the same context, whichever object the page built for it. */
export const contextKey = (context: PlaybackContext) =>
  'id' in context ? `${context.kind}:${context.id}` : context.kind

/** Track order of each sorted song table on screen, by `contextKey`, so playing its context follows what is shown. */
const shownOrders = new Map<string, TrackId[]>()

/** Makes playing `context` follow `order` until the returned function is called, as a sorted table does while it is shown. */
export function showOrder(context: PlaybackContext, order: TrackId[]) {
  const key = contextKey(context)
  shownOrders.set(key, order)
  return () => {
    if (shownOrders.get(key) === order) shownOrders.delete(key)
  }
}

function report(action: Promise<unknown>) {
  action.catch(reportError)
}

/** Player commands; outcomes arrive through `usePlayer`, failures as toasts. */
export const player = {
  play: (context: PlaybackContext, startTrackId: TrackId | null = null) =>
    report(
      // a playlist played moves up among the recently played in the sidebar
      unwrap(commands.play(context, startTrackId, shownOrders.get(contextKey(context)) ?? null)).then(async () => {
        if (context.kind === 'playlist') await invalidatePlaylists()
      }),
    ),
  /** Toggles when `context` is already playing, otherwise starts it from the top. */
  playOrToggle: (context: PlaybackContext) => {
    const { context: current } = usePlayer.getState()
    if (current !== null && sameContext(current, context)) player.toggle()
    else player.play(context)
  },
  enqueue: (trackIds: TrackId[]) =>
    report(
      unwrap(commands.enqueueTracks(trackIds)).then(() =>
        toast.success(`Added ${plural(trackIds.length, 'song')} to queue`),
      ),
    ),
  jump: (key: QueueKey) => report(commands.playQueueEntry(key)),
  remove: (key: QueueKey) => report(commands.removeFromQueue(key)),
  toggle: () => report(commands.togglePlayback()),
  next: () => report(commands.nextTrack()),
  previous: () => report(commands.previousTrack()),
  seek: (positionMs: number) => report(commands.seek(positionMs)),
  setVolume: (volume: number) => report(unwrap(commands.setVolume(volume))),
  setMuted: (muted: boolean) => report(commands.setMuted(muted)),
  setShuffle: (shuffle: boolean) => report(commands.setShuffle(shuffle)),
  setRepeat: (repeat: RepeatMode) => report(commands.setRepeat(repeat)),
  setFades: (fades: boolean) => report(commands.setFades(fades)),
  setCrossfade: (seconds: number) => report(unwrap(commands.setCrossfade(seconds))),
  setDiscordStatus: (shown: boolean) => report(commands.setDiscordStatus(shown)),
}

/** The longest crossfade in seconds, as the backend allows it. */
export const CROSSFADE_MAX = 12

/** Step of the repeat button: off, then the whole queue, then the current track. */
export const nextRepeat = { off: 'all', all: 'one', one: 'off' } as const satisfies Record<RepeatMode, RepeatMode>
