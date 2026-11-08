import { toast } from 'sonner'
import { create } from 'zustand'
import {
  type PlaybackContext,
  type PlayerState,
  type QueueKey,
  type RepeatMode,
  type TrackId,
  commands,
  events,
} from '@/bindings'
import { plural } from '@/lib/format'
import { unwrap } from '@/lib/ipc'

const initialState = await commands.getPlayerState()

/** Mirror of the backend player; Rust owns the state and pushes every change. */
export const usePlayer = create<PlayerState>()(() => initialState)

/** Subscribes the mirror to backend events, then re-reads the state to cover changes made before the subscription. */
export async function syncPlayer() {
  await Promise.all([
    events.playerStateChanged.listen(({ payload }) => usePlayer.setState(payload)),
    events.playbackProgress.listen(({ payload }) => usePlayer.setState(payload)),
  ])
  usePlayer.setState(await commands.getPlayerState())
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

function report(action: Promise<unknown>) {
  action.catch((error: unknown) => toast.error(error instanceof Error ? error.message : String(error)))
}

/** Player commands; outcomes arrive through `usePlayer`, failures as toasts. */
export const player = {
  play: (context: PlaybackContext, startTrackId: TrackId | null = null) =>
    report(unwrap(commands.play(context, startTrackId))),
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
  setShuffle: (shuffle: boolean) => report(commands.setShuffle(shuffle)),
  setRepeat: (repeat: RepeatMode) => report(commands.setRepeat(repeat)),
}

/** Step of the repeat button: off, then the whole queue, then the current track. */
export const nextRepeat = { off: 'all', all: 'one', one: 'off' } as const satisfies Record<RepeatMode, RepeatMode>
