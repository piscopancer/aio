import { create } from 'zustand'
import type { FoundSong } from '@/bindings'
import { player, usePlayer } from '@/lib/player'
import { queries, queryClient } from '@/lib/query'

/** Where the listening to a found song stands before it is downloaded. */
export type PreviewStatus = 'loading' | 'playing' | 'paused'

/** The found song being listened to in the download dialog, and how far it has got. */
const idle: {
  song: FoundSong | null
  status: PreviewStatus
  positionMs: number
  durationMs: number
} = { song: null, status: 'paused', positionMs: 0, durationMs: 0 }

export const usePreview = create<typeof idle>()(() => idle)

/** Whether the main player was playing when a preview took over, so it carries on once the preview stops. */
let resumeMain = false

/** Volume of the main player as the audio element takes it, from 0 to 1. */
const mainVolume = () => {
  const { volume, muted } = usePlayer.getState()
  return muted ? 0 : volume / 100
}

/** The one audio element previews play through, made on first use. */
let element: HTMLAudioElement | null = null

function audio() {
  if (element !== null) return element
  const made = new Audio()
  made.addEventListener('timeupdate', () => usePreview.setState({ positionMs: made.currentTime * 1000 }))
  made.addEventListener('durationchange', () => {
    if (Number.isFinite(made.duration)) usePreview.setState({ durationMs: made.duration * 1000 })
  })
  made.addEventListener('playing', () => usePreview.setState({ status: 'playing' }))
  // the finished song stays in the dialog's player, to be played again from its start
  made.addEventListener('ended', () => pausePreview())
  element = made
  return made
}

function pauseMain() {
  if (!usePlayer.getState().isPlaying) return
  resumeMain = true
  player.toggle()
}

function resumeMainIfPaused() {
  if (!resumeMain) return
  resumeMain = false
  if (!usePlayer.getState().isPlaying) player.toggle()
}

/** Whether `song` is the one the preview holds. */
const isCurrent = (song: FoundSong) => usePreview.getState().song?.videoUrl === song.videoUrl

/** Plays a found song, from where it paused when it is the one already loaded, pausing the main player meanwhile. */
export async function playPreview(song: FoundSong) {
  const sound = audio()
  sound.volume = mainVolume()
  pauseMain()
  if (isCurrent(song) && sound.src !== '') {
    await sound.play()
    return
  }
  usePreview.setState({ ...idle, song, status: 'loading' })
  try {
    const url = await queryClient.fetchQuery(queries.downloadStream(song.videoUrl))
    // another song was asked for, or the dialog closed, while the address was resolved
    if (!isCurrent(song)) return
    sound.src = url
    // the main player started by hand meanwhile, so the preview waits paused
    if (usePreview.getState().status === 'loading') await sound.play()
  } catch {
    // the query cache has reported the failure already
    if (isCurrent(song)) stopPreview()
  }
}

export function pausePreview() {
  audio().pause()
  usePreview.setState({ status: 'paused' })
  resumeMainIfPaused()
}

export function seekPreview(positionMs: number) {
  audio().currentTime = positionMs / 1000
  usePreview.setState({ positionMs })
}

/** Ends the preview and lets the main player carry on. */
export function stopPreview() {
  const sound = audio()
  sound.pause()
  sound.removeAttribute('src')
  sound.load()
  usePreview.setState(idle)
  resumeMainIfPaused()
}

usePlayer.subscribe((state, before) => {
  if (element !== null && (state.volume !== before.volume || state.muted !== before.muted))
    element.volume = mainVolume()
  // the main player started by hand takes over from the preview, and stays playing once the preview stops
  if (state.isPlaying && !before.isPlaying && usePreview.getState().status !== 'paused') {
    resumeMain = false
    audio().pause()
    usePreview.setState({ status: 'paused' })
  }
})
