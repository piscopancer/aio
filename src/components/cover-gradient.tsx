import { useQuery } from '@tanstack/react-query'
import { useLayoutEffect, useRef } from 'react'
import { queries } from '@/lib/query'
import { approach, createScene, playbackClock, runScene, useStill } from '@/lib/shader'
import { cn } from '@/lib/utils'

/** Colours the background blends, as many as the palette of a cover has. */
const PALETTE_SIZE = 4

/** Shades of grey the background flows with while nothing with a cover plays. */
const NEUTRAL_PALETTE = ['#2e2e2e', '#383838', '#262626', '#323232'] as const

/** The canvas is drawn at this share of its size on screen and stretched: the gradient has no detail to lose, and a
 * quarter of the pixels costs a quarter of the work. */
const RESOLUTION = 0.5

/** Seconds a new song's colours take to flow into the old ones. */
const BLEND_SECONDS = 1.2

/** Flowing swirls of the palette: the plane is warped by a few layers of waves, and every colour glows around a point
 * drifting on its own slow loop, so a full turn of the pattern takes about twenty to thirty seconds. The edges are
 * darkened a little, and a faint dither keeps the dark gradients from banding. */
const FRAGMENT_SHADER = `
precision mediump float;
uniform vec2 u_size;
uniform float u_time;
uniform vec3 u_colors[${PALETTE_SIZE}];

void main() {
  vec2 uv = gl_FragCoord.xy / u_size;
  float aspect = u_size.x / u_size.y;
  vec2 p = vec2((uv.x - 0.5) * aspect, uv.y - 0.5);
  float t = u_time;
  for (int i = 1; i <= 3; i++) {
    float f = float(i);
    p += 0.22 / f * vec2(sin(p.y * 2.1 * f + t * 0.23 + f * 1.3), cos(p.x * 1.9 * f - t * 0.19 + f * 2.1));
  }
  vec3 color = vec3(0.0);
  float total = 0.0;
  for (int i = 0; i < ${PALETTE_SIZE}; i++) {
    float f = float(i);
    vec2 center = vec2(0.45 * aspect * sin(t * 0.21 + f * 1.7), 0.42 * cos(t * 0.17 + f * 2.3));
    vec2 d = p - center;
    float weight = exp(-dot(d, d) * 3.5) + 0.002;
    color += u_colors[i] * weight;
    total += weight;
  }
  color /= total;
  vec2 edge = (uv - 0.5) * vec2(aspect, 1.0);
  color *= mix(0.78, 1.0, smoothstep(1.0, 0.25, length(edge)));
  float noise = fract(sin(dot(gl_FragCoord.xy, vec2(12.9898, 78.233))) * 43758.5453);
  gl_FragColor = vec4(color + (noise - 0.5) / 255.0, 1.0);
}`

/** The channels of a `#rrggbb` colour, from 0 to 1. */
const channels = (hex: string) => [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16) / 255)

/** The palette as the flat list of channels the shader takes. */
const flatten = (palette: readonly string[]) => palette.slice(0, PALETTE_SIZE).flatMap(channels)

/** A softly flowing gradient of the colours of `cover`, filling the nearest positioned parent behind its content. The
 * motion runs on the GPU in its own frame loop, never rendering React, slows to a halt while playback is paused and
 * stays still in the reduced motion mode; a new cover's colours flow into the old ones. */
export function CoverGradient({ cover, className }: { cover: string | null; className?: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { data: palette } = useQuery(queries.coverPalette(cover))
  const target = useRef<number[]>(flatten(NEUTRAL_PALETTE))
  const still = useStill()

  // a cover still being read keeps the colours shown so far rather than flashing grey; set before the frame loop below starts, so a cover already known shows at once
  useLayoutEffect(() => {
    if (cover === null) target.current = flatten(NEUTRAL_PALETTE)
    else if (palette != null) target.current = flatten(palette)
  }, [cover, palette])

  // before the browser paints, so the first frame shows the gradient rather than the stand-in colour
  useLayoutEffect(() => {
    const canvas = canvasRef.current
    const scene =
      canvas &&
      createScene(canvas, FRAGMENT_SHADER, ['u_time', 'u_colors'], {
        alpha: false,
        // kept after it is shown, so a repaint of what sits around it while the motion stands still never finds it cleared
        preserveDrawingBuffer: true,
      })
    if (canvas === null || scene == null) return undefined
    const shown = [...target.current]
    const clock = playbackClock(still)
    const stop = runScene(canvas, scene, RESOLUTION, (dt) => {
      let changed = clock.advance(dt)
      for (const [index, value] of target.current.entries()) {
        const before = shown[index] ?? value
        shown[index] = approach(before, value, dt, BLEND_SECONDS)
        if (Math.abs(before - value) > 1 / 512) changed = true
      }
      scene.gl.uniform1f(scene.uniforms.u_time, clock.time)
      scene.gl.uniform3fv(scene.uniforms.u_colors, shown)
      return changed
    })
    return () => {
      stop()
      // the webview keeps only a few contexts alive and drops the oldest, which may be the header's halftone, so a canvas that left the page lets its context go; one still in the page is only being set up again, as strict mode does in development, and keeps it
      setTimeout(() => {
        if (!canvas.isConnected) scene.gl.getExtension('WEBGL_lose_context')?.loseContext()
      })
    }
  }, [still])

  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      // the first palette colour stands in until the first frame, or for good without WebGL
      style={{ backgroundColor: palette?.[0] ?? NEUTRAL_PALETTE[0] }}
      className={cn('pointer-events-none absolute inset-0 -z-10 size-full', className)}
    />
  )
}
