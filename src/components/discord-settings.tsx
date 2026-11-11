import { SwitchField } from '@/components/settings-field'
import { player, usePlayer } from '@/lib/player'

/** Whether Discord shows the playing song on the user's profile. */
export function DiscordSettings() {
  const shown = usePlayer((state) => state.discordStatus)
  return (
    <SwitchField
      title="Show the playing song in Discord"
      hint="While a song plays, your Discord profile shows Listening to Aio with its title, artists and how far it has played. The status goes away when the music is paused or stops, and nothing is sent while Discord is closed."
      checked={shown}
      onCheckedChange={player.setDiscordStatus}
    />
  )
}
