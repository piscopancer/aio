import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import { HighlightStyle, StreamLanguage, forceParsing, syntaxHighlighting } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { EditorView, keymap } from '@codemirror/view'
import { tags } from '@lezer/highlight'
import { Maximize2Icon } from 'lucide-react'
import { type RefObject, useEffect, useEffectEvent, useRef, useState } from 'react'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { FieldLabel, type FieldLabelProps } from '@/components/ui/field-label'
import { cn } from '@/lib/utils'

/** A whole tag line such as `[ar: Artist]` or `[offset: +200]`. */
const TAG_LINE = /^\s*\[[a-z#]+:[^\]]*\]\s*$/i
/** A line stamp `[mm:ss.xx]` or a word stamp `<mm:ss.xx>`. */
const STAMP = /^(?:\[\d+:\d+(?:[.:]\d+)?\]|<\d+:\d+(?:[.:]\d+)?>)/

/** The bracket that closes the stamp or tag being read, if one is open. */
type LrcState = { closing: ']' | '>' | null }

/** LRC text, the brackets of its tag lines and time stamps marked as `bracket` and what they hold as `meta`, so both recede behind the words. */
const lrc = StreamLanguage.define<LrcState>({
  name: 'lrc',
  startState: () => ({ closing: null }),
  token: (stream, state) => {
    const { closing } = state
    if (closing !== null) {
      if (stream.eat(closing) !== undefined) {
        state.closing = null
        return 'bracket'
      }
      stream.eatWhile((char) => char !== closing)
      return 'meta'
    }
    if (stream.sol() && stream.match(TAG_LINE, false)) stream.eatSpace()
    else if (!stream.match(STAMP, false)) {
      // a bracket that opens no stamp is part of the words
      stream.next()
      stream.eatWhile(/[^[<]/)
      return null
    }
    state.closing = stream.next() === '<' ? '>' : ']'
    return 'bracket'
  },
})

/** Stamps and tag lines in a monospace face, so the stamps line up in a column, dim enough for the words to stand out, with what the brackets hold a shade lighter than the brackets. */
const lrcStyle = HighlightStyle.define([
  { tag: tags.bracket, class: 'font-mono text-subtle-foreground' },
  { tag: tags.meta, class: 'font-mono text-muted-foreground' },
])

/** Gives the editor the app's font, the browser's own caret and selection, the padding around the text and the least height of its typing area, which takes the clicks; CodeMirror's own least height of 100% only fills an editor of a set height. */
const editorTheme = (padding: string, minHeight = '100%') =>
  EditorView.theme({
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': { fontFamily: 'inherit', lineHeight: 'inherit' },
    '.cm-content': { padding, minHeight, caretColor: 'var(--foreground)' },
    '.cm-line': { padding: '0' },
  })

/** How the editor sits: in the form as one more text area, at least as high as the others and at most as they allow, or filling the large window bare, with a larger font and its text in line with the window's title. */
const SIZES = {
  field: {
    className:
      'rounded-md border border-transparent bg-secondary text-sm has-[.cm-focused]:border-input has-[.cm-focused]:bg-accent [&_.cm-editor]:max-h-64',
    theme: editorTheme('0.625rem 0.75rem', '5rem'),
  },
  large: { className: 'min-h-0 flex-1 text-lg [&_.cm-editor]:h-full', theme: editorTheme('1rem 1.5rem') },
} as const

/** How long the whole text may take to colour at once; lyrics are short, so this is only a guard against a huge paste. */
const PARSE_BUDGET_MS = 500

/** Colours the whole text at once, where the editor would otherwise colour it bit by bit as it scrolls into view. */
const parseWhole = (editor: EditorView) => forceParsing(editor, editor.state.doc.length, PARSE_BUDGET_MS)

/** Text area for LRC lyrics with their time stamps and tag lines dimmed, so the sung words stand out, and a button that opens them in a large window. */
export function LrcField({
  label,
  icon,
  hint,
  value,
  onChange,
  className,
}: FieldLabelProps & { label: string; value: string; onChange: (value: string) => void; className?: string }) {
  const view = useRef<EditorView>(null)
  const large = useRef<EditorView>(null)
  const [expanded, setExpanded] = useState(false)
  /** Hands the form what the large window holds once it closes, so typing there renders nothing but the editor itself. */
  const expand = (open: boolean) => {
    const text = large.current?.state.doc.toString()
    if (!open && text !== undefined && text !== value) onChange(text)
    setExpanded(open)
  }
  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      <div className="flex items-center justify-between gap-2">
        {/* the editor is no form control a label can point to, so a click on the label focuses it by hand */}
        <FieldLabel icon={icon} hint={hint} onClick={() => view.current?.focus()}>
          {label}
        </FieldLabel>
        {/* in the label's own type and line height, so the row stands as high as the labels of the other fields */}
        <button
          type="button"
          onClick={() => setExpanded(true)}
          className="flex cursor-pointer items-center gap-1.5 text-sm font-semibold text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:text-foreground"
        >
          <Maximize2Icon className="size-3.5 shrink-0" />
          Expand
        </button>
      </div>
      <LrcEditor label={label} value={value} onChange={onChange} size="field" viewRef={view} />
      <Dialog open={expanded} onOpenChange={expand}>
        <DialogContent
          className="h-[calc(100dvh-4rem)] max-w-xl"
          // the editor rather than the close button, so typing goes on where it left off
          onOpenAutoFocus={(event) => {
            event.preventDefault()
            large.current?.focus()
          }}
        >
          <DialogHeader>
            <DialogTitle>{label}</DialogTitle>
          </DialogHeader>
          <LrcEditor label={label} value={value} size="large" viewRef={large} />
        </DialogContent>
      </Dialog>
    </div>
  )
}

/** The CodeMirror editor itself, kept in step with `value`; `viewRef` gets the editor, to focus it or read its text from outside, and `onChange` hears of every edit. */
function LrcEditor({
  label,
  value,
  onChange,
  size,
  viewRef,
}: {
  label: string
  value: string
  onChange?: (value: string) => void
  size: keyof typeof SIZES
  viewRef: RefObject<EditorView | null>
}) {
  const host = useRef<HTMLDivElement>(null)
  const changed = useEffectEvent((text: string) => onChange?.(text))

  useEffect(() => {
    if (host.current === null) return
    const editor = new EditorView({
      parent: host.current,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          keymap.of([...defaultKeymap, ...historyKeymap]),
          lrc,
          syntaxHighlighting(lrcStyle),
          EditorView.lineWrapping,
          SIZES[size].theme,
          EditorView.contentAttributes.of({ 'aria-label': label }),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) changed(update.state.doc.toString())
          }),
        ],
      }),
    })
    viewRef.current = editor
    parseWhole(editor)
    return () => {
      editor.destroy()
      viewRef.current = null
    }
    // made once, keeping its own text, which the effect below brings in line with `value`
  }, [])

  // text set from outside, as by a found match or the other editor of the same field, replaces what the editor holds
  useEffect(() => {
    const editor = viewRef.current
    if (editor === null) return
    const held = editor.state.doc.toString()
    if (held === value) return
    editor.dispatch({ changes: { from: 0, to: held.length, insert: value } })
    parseWhole(editor)
  }, [value, viewRef])

  return <div ref={host} className={cn('min-w-0 overflow-hidden select-text', SIZES[size].className)} />
}
