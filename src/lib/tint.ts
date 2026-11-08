/** Golden angle in degrees, so neighbouring ids land on clearly different hues. */
const GOLDEN_ANGLE = 137.5

/** Header colour of a page: a stable hue derived from the entity id. */
export function tint(seed: number) {
  return `oklch(0.46 0.1 ${(seed * GOLDEN_ANGLE) % 360})`
}

/** Header colour of pages that show the whole library, a darker shade of the accent. */
export const libraryTint = 'oklch(0.5 0.09 62)'
