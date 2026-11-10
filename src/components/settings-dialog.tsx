import { DownloadIcon, SlidersHorizontalIcon } from 'lucide-react'
import type { ReactNode } from 'react'
import { DiscordSettings } from '@/components/discord-settings'
import { DownloadSettings } from '@/components/download-settings'
import { PlaybackSettings } from '@/components/playback-settings'
import { UpdateSection } from '@/components/update-section'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'

/** General tab: how songs fade and whether Discord shows them, then, set apart below, the app version and its updates. */
function GeneralSettings() {
  return (
    <div className="grid gap-6">
      <PlaybackSettings />
      <DiscordSettings />
      <hr />
      <UpdateSection />
    </div>
  )
}

/** Sections of the settings dialog, in the order the tab list shows them. */
const sections = [
  { value: 'general', label: 'General', icon: SlidersHorizontalIcon, content: GeneralSettings },
  { value: 'downloads', label: 'Downloads', icon: DownloadIcon, content: DownloadSettings },
] as const

/** App settings, opened by the given trigger. */
export function SettingsDialog({ children }: { children: ReactNode }) {
  return (
    <Dialog>
      <DialogTrigger asChild>{children}</DialogTrigger>
      <DialogContent aria-describedby={undefined} className="h-[calc(100dvh-4rem)] max-w-4xl bg-background">
        <DialogHeader>
          <DialogTitle>Settings</DialogTitle>
        </DialogHeader>
        <Tabs orientation="vertical" defaultValue={sections[0].value} className="min-h-0 flex-1 p-6">
          <TabsList className="w-44 shrink-0">
            {sections.map(({ value, label, icon: Icon }) => (
              <TabsTrigger key={value} value={value}>
                <Icon />
                {label}
              </TabsTrigger>
            ))}
          </TabsList>
          {sections.map(({ value, label, content: Content }) => (
            <TabsContent key={value} value={value} className="overflow-y-auto rounded-md bg-surface p-5">
              <h3 className="mb-4 text-lg font-bold">{label}</h3>
              <Content />
            </TabsContent>
          ))}
        </Tabs>
      </DialogContent>
    </Dialog>
  )
}
