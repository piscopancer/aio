import { getCurrentWindow } from '@tauri-apps/api/window'
import { useRouter } from '@tanstack/react-router'
import { useEffect } from 'react'
import { SEARCH_INPUT_ID } from '@/components/search'
import { togglePanel, toggleSidebar } from '@/lib/layout'
import { nextRepeat, player, usePlayer } from '@/lib/player'

/** `MouseEvent.button` values of the side buttons most mice have. */
const MouseButton = { back: 3, forward: 4 } as const

/** Whether the key press lands in a field the user types into. */
function editsText(target: EventTarget | null) {
  return target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement
}

/** Whether the key press belongs to the focused element, which gives Space its own meaning. */
function ownsSpace(target: EventTarget | null) {
  return (
    target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLButtonElement
  )
}

/** Switches the window between full screen and its normal frame. */
async function toggleFullscreen() {
  const window = getCurrentWindow()
  await window.setFullscreen(!(await window.isFullscreen()))
}

/** Browser-like window shortcuts and no native context menu; zoom (Ctrl + wheel, Ctrl + plus, minus, 0) is the webview's own, see `zoomHotkeysEnabled`. */
export function useAppShortcuts() {
  const { history } = useRouter()

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat) return
      if (event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
        event.preventDefault()
        if (event.key === 'ArrowLeft') history.back()
        else history.forward()
        return
      }
      // the key's place, not its letter, so the shortcut works in any keyboard layout
      if (event.ctrlKey && !event.shiftKey && !event.altKey && event.code === 'KeyB') {
        event.preventDefault()
        toggleSidebar()
        return
      }
      if (event.key === 'F11') {
        event.preventDefault()
        void toggleFullscreen()
        return
      }
      const ctrlOnly = event.ctrlKey && !event.shiftKey && !event.altKey
      // in a text field Ctrl with an arrow moves the caret by words, so the player leaves it alone there
      if (ctrlOnly && (event.key === 'ArrowLeft' || event.key === 'ArrowRight') && !editsText(event.target)) {
        event.preventDefault()
        if (event.key === 'ArrowLeft') player.previous()
        else player.next()
        return
      }
      if (ctrlOnly && event.code === 'KeyM') {
        event.preventDefault()
        player.setMuted(!usePlayer.getState().muted)
        return
      }
      if (ctrlOnly && event.code === 'KeyR') {
        event.preventDefault()
        player.setRepeat(nextRepeat[usePlayer.getState().repeat])
        return
      }
      if (event.ctrlKey && !event.shiftKey && !event.altKey && event.code === 'KeyL') {
        event.preventDefault()
        togglePanel('lyrics')
        return
      }
      if (event.ctrlKey && !event.shiftKey && !event.altKey && event.code === 'KeyF') {
        event.preventDefault()
        const search = document.getElementById(SEARCH_INPUT_ID)
        if (search instanceof HTMLInputElement) {
          search.focus()
          search.select()
        }
        return
      }
      if (event.code === 'Space' && !event.altKey && !event.ctrlKey && !ownsSpace(event.target)) {
        event.preventDefault()
        player.toggle()
      }
    }
    const onMouseUp = (event: MouseEvent) => {
      if (event.button === MouseButton.back) history.back()
      else if (event.button === MouseButton.forward) history.forward()
      else return
      event.preventDefault()
    }
    // the webview's own menu (save link, inspect) makes no sense in the app; text fields keep it for copy and paste, and dev builds keep it behind Shift for the inspector
    const onContextMenu = (event: MouseEvent) => {
      const editable = event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement
      if (!editable && !(import.meta.env.DEV && event.shiftKey)) event.preventDefault()
    }
    window.addEventListener('keydown', onKeyDown)
    window.addEventListener('mouseup', onMouseUp)
    window.addEventListener('contextmenu', onContextMenu)
    return () => {
      window.removeEventListener('keydown', onKeyDown)
      window.removeEventListener('mouseup', onMouseUp)
      window.removeEventListener('contextmenu', onContextMenu)
    }
  }, [history])
}
