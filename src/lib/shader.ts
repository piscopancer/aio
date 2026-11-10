import { useReducedMotion } from 'motion/react'
import { useRef } from 'react'
import { usePlayer } from '@/lib/player'

/** Draws the whole canvas with one triangle that covers it. */
const VERTEX_SHADER = `
attribute vec2 a_position;
void main() { gl_Position = vec4(a_position, 0.0, 1.0); }`

/** Seconds the motion takes to come to a halt on pause, or back to its pace on play. */
const EASE_SECONDS = 1.5

/** Milliseconds between pictures of a moving scene: 30 a second is smooth enough for slow motion, and every picture left out is a frame the webview composes without a fresh canvas, which keeps a keystroke from waiting behind the shaders. */
const FRAME_MS = 1000 / 30

/** Moves `value` toward `target` by the share of the way an exponential ease of `seconds` covers in `dt` seconds. */
export const approach = (value: number, target: number, dt: number, seconds: number) =>
  value + (target - value) * (1 - Math.exp((-dt * 3) / seconds))

function compile(gl: WebGLRenderingContext, type: GLenum, source: string) {
  const shader = gl.createShader(type)
  if (shader === null) return null
  gl.shaderSource(shader, source)
  gl.compileShader(shader)
  return shader
}

/** The full-canvas fragment shader `source` on `canvas` with the locations of its `uniforms` and of `u_size`, the canvas size in its own pixels, or `null` when the webview has no WebGL to give. */
export function createScene<const U extends string>(
  canvas: HTMLCanvasElement,
  source: string,
  uniforms: readonly U[],
  attributes: WebGLContextAttributes,
) {
  const gl = canvas.getContext('webgl', { antialias: false, depth: false, ...attributes })
  if (gl === null) return null
  const program = gl.createProgram()
  const vertex = compile(gl, gl.VERTEX_SHADER, VERTEX_SHADER)
  const fragment = compile(gl, gl.FRAGMENT_SHADER, source)
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
  // Object.fromEntries widens the keys to string, while they are exactly the names asked for
  const locations = Object.fromEntries(uniforms.map((name) => [name, gl.getUniformLocation(program, name)])) as Record<
    U,
    WebGLUniformLocation | null
  >
  return { gl, size: gl.getUniformLocation(program, 'u_size'), uniforms: locations }
}

/** A scene `createScene` set up. */
export type Scene<U extends string> = NonNullable<ReturnType<typeof createScene<U>>>

/** Runs the frame loop of `scene` on `canvas`, drawn at `resolution` of its size on screen; `frame` gets the seconds since the last frame, sets the uniforms and tells whether the picture changed; a still picture is drawn again only on a resize, and the first one before it returns. Returns the function that stops it. */
export function runScene<U extends string>(
  canvas: HTMLCanvasElement,
  scene: Scene<U>,
  resolution: number,
  frame: (dt: number) => boolean,
) {
  const { gl } = scene
  const render = () => gl.drawArrays(gl.TRIANGLES, 0, 3)
  // the size this run last gave the scene; a canvas set up again, as strict mode and a change of `still` do, already has its size while the new program does not know it yet, so the canvas's own size cannot tell
  let size = { width: 0, height: 0 }
  // a new size clears the canvas, so it is drawn again at once rather than left blank until the next frame
  const resize = () => {
    const width = Math.max(1, Math.round(canvas.clientWidth * resolution))
    const height = Math.max(1, Math.round(canvas.clientHeight * resolution))
    if (width === size.width && height === size.height) return
    size = { width, height }
    canvas.width = width
    canvas.height = height
    gl.viewport(0, 0, width, height)
    gl.uniform2f(scene.size, width, height)
    render()
  }

  let last = performance.now()
  let request = 0
  const draw = (now: number) => {
    request = requestAnimationFrame(draw)
    // a hair short of the interval, so the jitter of the display's frames never pushes a picture to the frame after
    if (now - last < FRAME_MS - 2) return
    const dt = Math.min((now - last) / 1000, 0.1)
    last = now
    if (frame(dt)) render()
  }
  // the first picture is drawn at once rather than on the next animation frame, so a layout effect gets it painted with the page
  frame(0)
  resize()
  render()
  const observer = new ResizeObserver(resize)
  observer.observe(canvas)
  request = requestAnimationFrame(draw)
  return () => {
    cancelAnimationFrame(request)
    observer.disconnect()
  }
}

/** Whether shader motion should stand still, kept in a ref so a running frame loop reads the current answer. */
export function useStill() {
  const reducedMotion = useReducedMotion() ?? false
  const still = useRef(reducedMotion)
  still.current = reducedMotion
  return still
}

/** Time of a pattern that moves while music plays: it eases to a halt on pause or while `still` holds, and back to its pace on play; it starts somewhere along its loop, so not every opening looks the same. */
export function playbackClock(still: { readonly current: boolean }) {
  let time = Math.random() * 100
  let speed = usePlayer.getState().isPlaying ? 1 : 0
  return {
    /** Moves the time on by `dt` seconds and tells whether it is still moving. */
    advance(dt: number) {
      const playing = usePlayer.getState().isPlaying && !still.current
      speed = approach(speed, playing ? 1 : 0, dt, EASE_SECONDS)
      time += dt * speed
      return speed > 1e-3
    },
    get time() {
      return time
    },
  }
}
