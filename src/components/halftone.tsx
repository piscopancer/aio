import { useLayoutEffect, useRef } from 'react'
import { type Scene, createScene, playbackClock, runScene, useStill } from '@/lib/shader'
import { cn } from '@/lib/utils'

/** CSS pixels between the centres of neighbouring dots. */
const PITCH = 8

/** Dot coverage at the top and at the bottom of the canvas, from 0 (no dot) to 1 (dots merge into a solid fill). */
const TONE = { top: 0.9, bottom: 0 } as const

/** How far the drifting waves push the coverage up and down at the top, and the share of the height from the top at which they have died out, so the lower part fades in straight rows. */
const RIPPLE = { strength: 0.12, until: 0.55 } as const

/** Halftone print over whatever lies behind: the canvas is filled with `u_color` except for round holes on a grid turned by 45 degrees, and the holes shrink from top to bottom, so the colour behind shows through as dots that fade out downward. Two slow waves in CSS pixels ripple the dot size in the upper part only, and the edges are smoothed over about one device pixel. */
const FRAGMENT_SHADER = `
precision highp float;
uniform vec2 u_size;
uniform float u_time;
uniform float u_scale;
uniform vec3 u_color;

void main() {
  vec2 css = vec2(gl_FragCoord.x, u_size.y - gl_FragCoord.y) / u_scale;
  float y = 1.0 - gl_FragCoord.y / u_size.y;
  float t = u_time;
  float ripple = 0.5 * sin(css.x * 0.011 + css.y * 0.014 + t * 0.4) + 0.5 * sin(css.y * 0.009 - css.x * 0.017 - t * 0.31 + 1.7);
  float base = mix(${TONE.top.toFixed(3)}, ${TONE.bottom.toFixed(3)}, smoothstep(0.0, 1.0, y));
  float calm = 1.0 - smoothstep(0.0, ${RIPPLE.until.toFixed(3)}, y);
  float tone = clamp(base + ${RIPPLE.strength.toFixed(3)} * ripple * calm, 0.0, 1.0);
  vec2 grid = mat2(0.70710678, -0.70710678, 0.70710678, 0.70710678) * css / ${PITCH.toFixed(1)};
  float d = length(fract(grid) - 0.5);
  float radius = 0.72 * sqrt(tone);
  float edge = 0.6 / (${PITCH.toFixed(1)} * u_scale);
  float alpha = smoothstep(radius - edge, radius + edge, d);
  gl_FragColor = vec4(u_color * alpha, alpha);
}`

/** The channels of a computed `rgb(r, g, b)` colour, from 0 to 1. */
const channels = (rgb: string) => (rgb.match(/[\d.]+/g) ?? []).slice(0, 3).map((channel) => Number(channel) / 255)

/** Uniforms the shader takes besides the canvas size. */
const UNIFORMS = ['u_time', 'u_scale', 'u_color'] as const

/** Whether the dots of the header that shows the canvas stand still, read by the clock on every frame. */
type Owner = { readonly still: { readonly current: boolean } }

/** The app's one halftone canvas with its scene and clock, made on first use; it is moved into whichever header shows it, so the shader is compiled once and the waves carry on from page to page. */
type Shared = {
  canvas: HTMLCanvasElement
  scene: Scene<(typeof UNIFORMS)[number]> | null
  clock: ReturnType<typeof playbackClock>
  owner: Owner | null
}

let shared: Shared | undefined

function sharedHalftone() {
  if (shared !== undefined) return shared
  const canvas = document.createElement('canvas')
  canvas.setAttribute('aria-hidden', 'true')
  canvas.className = 'block size-full'
  const made: Shared = {
    canvas,
    // the picture is kept after it is shown: when the header behind repaints, as its colour comes in, the webview may paint the canvas again from it, and a cleared buffer would show the bare colour for a frame
    scene: createScene(canvas, FRAGMENT_SHADER, UNIFORMS, { alpha: true, preserveDrawingBuffer: true }),
    clock: playbackClock({
      get current() {
        return made.owner?.still.current ?? true
      },
    }),
    owner: null,
  }
  // a context the GPU took back is gone for good, so the next header makes a fresh canvas
  canvas.addEventListener('webglcontextlost', () => {
    if (shared === made) shared = undefined
  })
  shared = made
  return made
}

/** Halftone dots of the colours behind it, filling the nearest positioned parent behind its content; the gaps take the text colour of `className`, the surface by default. The dots ripple slowly while music plays, ease to a halt on pause and stay still in the reduced motion mode; without WebGL nothing covers the colours. */
export function Halftone({ className }: { className?: string }) {
  const slotRef = useRef<HTMLDivElement>(null)
  const still = useStill()

  // before the browser paints, so the page never shows its colours without the dots over them
  useLayoutEffect(() => {
    const slot = slotRef.current
    const halftone = sharedHalftone()
    const { canvas, scene, clock } = halftone
    if (slot === null || scene === null) return undefined
    slot.append(canvas)
    const owner: Owner = { still }
    halftone.owner = owner
    const { gl, uniforms } = scene
    const resolution = window.devicePixelRatio
    gl.uniform1f(uniforms.u_scale, resolution)
    gl.uniform3fv(uniforms.u_color, channels(getComputedStyle(canvas).color))
    const stop = runScene(canvas, scene, resolution, (dt) => {
      const moving = clock.advance(dt)
      gl.uniform1f(uniforms.u_time, clock.time)
      return moving
    })
    return () => {
      stop()
      // a header that mounted meanwhile has taken the canvas over, and keeps it
      if (halftone.owner === owner) halftone.owner = null
    }
  }, [still])

  return <div ref={slotRef} className={cn('pointer-events-none absolute inset-0 -z-10 text-surface', className)} />
}
