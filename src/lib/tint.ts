/** Header colour of a page without cover art, a plain dark grey. */
export const neutralTint = 'oklch(0.4 0 0)'

/** Lightness range a header glow is held in: dark covers are lifted so they still glow, light ones capped so white titles stay readable. */
const GLOW_LIGHTNESS = { min: 0.45, max: 0.7 } as const

/** Header glow of a cover colour: the colour itself, with its lightness held in `GLOW_LIGHTNESS`. */
export const glowOf = (color: string) =>
  `oklch(from ${color} clamp(${GLOW_LIGHTNESS.min}, l, ${GLOW_LIGHTNESS.max}) c h)`

/** Header colour of pages that show the whole library, whose cover is the accent: its glow, as a cover of that colour would give. */
export const libraryTint = glowOf('var(--primary)')
