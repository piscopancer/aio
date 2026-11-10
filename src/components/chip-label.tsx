import { Marquee } from '@/components/marquee'

/** The text of a capsule, such as an artist or a genre: one short width for every capsule, running as a marquee when the text is longer. */
export function ChipLabel({ children }: { children: string }) {
  return (
    <Marquee play="always" className="max-w-36">
      {children}
    </Marquee>
  )
}
