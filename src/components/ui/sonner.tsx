import type { CSSProperties } from 'react'
import { Toaster as Sonner, type ToasterProps } from 'sonner'

/** Theme variables of sonner mapped onto the app tokens, and the app's font, which sonner's own stylesheet replaces with the system one. */
const style = {
  fontFamily: 'inherit',
  '--normal-bg': 'var(--popover)',
  '--normal-text': 'var(--popover-foreground)',
  '--normal-border': 'var(--border)',
} as CSSProperties // custom properties are missing from CSSProperties

function Toaster(props: ToasterProps) {
  return <Sonner theme="dark" position="bottom-center" offset={104} style={style} {...props} />
}

export { Toaster }
