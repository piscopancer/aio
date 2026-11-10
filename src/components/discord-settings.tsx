import { Field } from '@/components/settings-field'
import { Checkbox } from '@/components/ui/checkbox'
import { player, usePlayer } from '@/lib/player'

/** Whether Discord shows the playing song on the user's profile. */
export function DiscordSettings() {
  const shown = usePlayer((state) => state.discordStatus)
  return (
    <Field
      title="Discord"
      hint="While a song plays, your Discord profile shows Listening to Aio with its title, artists and how far it has played. The status goes away when the music is paused or stops, and nothing is sent while Discord is closed."
    >
      <label className="flex w-fit items-center gap-2 text-sm">
        <Checkbox checked={shown} onCheckedChange={(checked) => player.setDiscordStatus(checked === true)} />
        Show the playing song in Discord
      </label>
    </Field>
  )
}
