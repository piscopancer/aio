import { useRouter } from '@tanstack/react-router'
import { useEffect } from 'react'
import { toggleSidebar } from '@/lib/layout'
import { player } from '@/lib/player'

/** `MouseEvent.button` values of the side buttons most mice have. */
const MouseButton = { back: 3, forward: 4 } as const

/** Whether the key press belongs to the focused element, which gives Space its own meaning. */
function ownsSpace(target: EventTarget | null) {
  return (
    target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLButtonElement
  )
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
