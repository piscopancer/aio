import { Field } from '@/components/settings-field'
import { Checkbox } from '@/components/ui/checkbox'
import { Slider } from '@/components/ui/slider'
import { CROSSFADE_MAX, player, usePlayer } from '@/lib/player'

/** How songs fade as they start, stop, pause and resume, and how far one song plays over the next. */
export function PlaybackSettings() {
  const fades = usePlayer((state) => state.fades)
  const crossfade = usePlayer((state) => state.crossfade)
  return (
    <>
      <Field
        title="Fades"
        hint="Songs fade in and out over half a second as they start, stop, pause and resume, instead of cutting in and off."
      >
        <label className="flex w-fit items-center gap-2 text-sm">
          <Checkbox checked={fades} onCheckedChange={(checked) => player.setFades(checked === true)} />
          Fade songs in and out
        </label>
      </Field>
      <Field
        title="Crossfade"
        hint="The next song starts while the current one fades out, so the music never stops between them. Songs of one album follow each other without it, as such albums are often meant to play without gaps."
      >
        <div className="flex items-center gap-4">
          <Slider
            aria-label="Crossfade"
            min={0}
            max={CROSSFADE_MAX}
            step={1}
            value={[crossfade]}
            onValueChange={([seconds]) => seconds !== undefined && player.setCrossfade(seconds)}
            className="max-w-72"
          />
          <span className="w-10 text-sm text-muted-foreground tabular-nums">
            {crossfade === 0 ? 'Off' : `${crossfade} s`}
          </span>
        </div>
      </Field>
    </>
  )
}
