import { useMutation, useQuery } from '@tanstack/react-query'
import { SwitchField } from '@/components/settings-field'
import { mutations, queries } from '@/lib/query'

/** Whether the app starts with Windows, whether that start stays in the tray, and whether closing the window hides it there. */
export function TraySettings() {
  const { data: autostart = false } = useQuery(queries.autostart())
  const { data: settings } = useQuery(queries.windowSettings())
  const setAutostart = useMutation(mutations.setAutostart())
  const setStartHidden = useMutation(mutations.setStartHidden())
  const setCloseToTray = useMutation(mutations.setCloseToTray())
  return (
    <>
      <SwitchField
        title="Start with Windows"
        hint="Aio starts when you sign in to Windows, so the music is a click away."
        checked={autostart}
        disabled={setAutostart.isPending}
        onCheckedChange={setAutostart.mutate}
      />
      <SwitchField
        title="Start in the tray"
        hint="When Windows starts Aio at sign-in, it waits in the tray instead of opening its window. Opening Aio yourself always shows the window. Works together with Start with Windows."
        checked={settings?.startHidden ?? false}
        disabled={!autostart || settings === undefined}
        onCheckedChange={setStartHidden.mutate}
      />
      <SwitchField
        title="Close to the tray"
        hint="Closing the window hides it to the tray and the music plays on. Click the tray icon to bring the window back, or right-click it to control the music or quit."
        checked={settings?.closeToTray ?? false}
        disabled={settings === undefined}
        onCheckedChange={setCloseToTray.mutate}
      />
    </>
  )
}
