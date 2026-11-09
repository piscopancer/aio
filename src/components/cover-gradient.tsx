import { useQuery } from '@tanstack/react-query'
import { useReducedMotion } from 'motion/react'
import { useEffect, useRef } from 'react'
import { usePlayer } from '@/lib/player'
import { queries } from '@/lib/query'
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

/** Seconds the motion takes to come to a halt on pause, or back to its pace on play. */
const EASE_SECONDS = 1.5

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

/** Draws the whole canvas with one triangle that covers it. */
const VERTEX_SHADER = `
attribute vec2 a_position;
void main() { gl_Position = vec4(a_position, 0.0, 1.0); }`

/** The channels of a `#rrggbb` colour, from 0 to 1. */
const channels = (hex: string) => [1, 3, 5].map((at) => Number.parseInt(hex.slice(at, at + 2), 16) / 255)

/** The palette as the flat list of channels the shader takes. */
const flatten = (palette: readonly string[]) => palette.slice(0, PALETTE_SIZE).flatMap(channels)

/** Moves `value` toward `target` by the share of the way an exponential ease of `seconds` covers in `dt` seconds. */
const approach = (value: number, target: number, dt: number, seconds: number) =>
  value + (target - value) * (1 - Math.exp((-dt * 3) / seconds))

function compile(gl: WebGLRenderingContext, type: GLenum, source: string) {
  const shader = gl.createShader(type)
  if (shader === null) return null
  gl.shaderSource(shader, source)
  gl.compileShader(shader)
  return shader
}

/** The shader program on `canvas`, or `null` when the webview has no WebGL to give. */
function setUp(canvas: HTMLCanvasElement) {
  const gl = canvas.getContext('webgl', { antialias: false, depth: false, alpha: false })
  if (gl === null) return null
  const program = gl.createProgram()
  const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX_SHADER)
  const fragment = compile(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER)
  if (vertex === null || fragment === null) return null
  gl.attachShader(program, vertex)
  gl.attachShader(program, fragment)
  gl.linkProgram(program)
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) return null
  gl.useProgram(program)
  gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer())
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW)
  const position = gl.getAttribLocation(program, 'a_position')
  gl.enableVertexAttribArray(position)
  gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0)
  return {
    gl,
    size: gl.getUniformLocation(program, 'u_size'),
    time: gl.getUniformLocation(program, 'u_time'),
    colors: gl.getUniformLocation(program, 'u_colors'),
  }
}

/** A softly flowing gradient of the colours of `cover`, filling the nearest positioned parent behind its content. The
 * motion runs on the GPU in its own frame loop, never rendering React, slows to a halt while playback is paused and
 * stays still in the reduced motion mode; a new cover's colours flow into the old ones. */
export function CoverGradient({ cover, className }: { cover: string | null; className?: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const { data: palette } = useQuery(queries.coverPalette(cover))
  const target = useRef<number[]>(flatten(NEUTRAL_PALETTE))
  const reducedMotion = useReducedMotion() ?? false
  const still = useRef(reducedMotion)
  still.current = reducedMotion

  // a cover still being read keeps the colours shown so far rather than flashing grey
  useEffect(() => {
    if (cover === null) target.current = flatten(NEUTRAL_PALETTE)
    else if (palette != null) target.current = flatten(palette)
  }, [cover, palette])

  useEffect(() => {
    const canvas = canvasRef.current
    const scene = canvas && setUp(canvas)
    if (canvas === null || scene == null) return undefined
    const { gl } = scene
    const shown = [...target.current]
    // the pattern starts somewhere along its loop, so every opening of the lyrics does not look the same
    let time = Math.random() * 100
    let speed = usePlayer.getState().isPlaying ? 1 : 0
    let last = performance.now()
    let frame = 0
    // a still picture is drawn once rather than on every frame
    let stale = true

    const resize = () => {
      canvas.width = Math.max(1, Math.round(canvas.clientWidth * RESOLUTION))
      canvas.height = Math.max(1, Math.round(canvas.clientHeight * RESOLUTION))
      gl.viewport(0, 0, canvas.width, canvas.height)
      gl.uniform2f(scene.size, canvas.width, canvas.height)
      stale = true
    }
    const observer = new ResizeObserver(resize)
    observer.observe(canvas)
    resize()

    const draw = (now: number) => {
      const dt = Math.min((now - last) / 1000, 0.1)
      last = now
      const playing = usePlayer.getState().isPlaying && !still.current
      speed = approach(speed, playing ? 1 : 0, dt, EASE_SECONDS)
      time += dt * speed
      for (const [index, value] of target.current.entries()) {
        const before = shown[index] ?? value
        shown[index] = approach(before, value, dt, BLEND_SECONDS)
        if (Math.abs(before - value) > 1 / 512) stale = true
      }
      if (stale || speed > 1e-3) {
        gl.uniform1f(scene.time, time)
        gl.uniform3fv(scene.colors, shown)
        gl.drawArrays(gl.TRIANGLES, 0, 3)
        stale = false
      }
      frame = requestAnimationFrame(draw)
    }
    frame = requestAnimationFrame(draw)
    return () => {
      cancelAnimationFrame(frame)
      observer.disconnect()
    }
  }, [])

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
