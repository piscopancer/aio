import { RowField, SwitchField } from '@/components/settings-field'
import { Slider } from '@/components/ui/slider'
import { CROSSFADE_MAX, player, usePlayer } from '@/lib/player'

/** How songs fade as they start, stop, pause and resume, and how far one song plays over the next. */
export function PlaybackSettings() {
  const fades = usePlayer((state) => state.fades)
  const crossfade = usePlayer((state) => state.crossfade)
  return (
    <>
      <SwitchField
        title="Fade songs in and out"
        hint="Songs fade in and out over half a second as they start, stop, pause and resume, instead of cutting in and off."
        checked={fades}
        onCheckedChange={player.setFades}
      />
      <RowField
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
          />
          <span className="w-10 text-sm text-muted-foreground tabular-nums">
            {crossfade === 0 ? 'Off' : `${crossfade} s`}
          </span>
        </div>
      </RowField>
    </>
  )
}
