import { useEffect, useRef, useState, type CSSProperties } from 'react'
import { clock } from '../../kit'
import './look.css'
import './meter.css'

// The floating meter, at the geometry and colours of rust/src/overlay.rs:
// a 336 × 52 pill, 40 bars 5px apart from x=40, the dot or icon centered at
// x=26, the text 18px from the ends. Recording: a red dot with a pulsing
// halo, the level history as bars (new levels push in from the right), the
// timer on the right. Paused: an amber pause mark and the word Paused.
// Working: "Transcribing" on the left and a glow that runs along the bars.

export type MeterState = 'hidden' | 'recording' | 'paused' | 'working'

const W = 336
const H = 52
const BARS = 40
const PITCH = 5
const BAR_W = 3
const MAX_BAR = 15
const BARS_W = (BARS - 1) * PITCH + BAR_W // the bars' own box, placed at x=40
const WORK_START = 14 // the first bar right of the Transcribing label
const FRAME_MS = 33

type Rgb = [number, number, number]
const IDLE: Rgb = [58, 61, 71]
const LOW: Rgb = [56, 189, 160]
const HIGH: Rgb = [125, 211, 252]
const REC = '#f05454'
const AMBER = '#f5b942'

const mix = (a: Rgb, b: Rgb, t: number) => `rgb(${a.map((v, i) => Math.round(v + (b[i] - v) * t)).join(' ')})`
const fract = (x: number) => x - Math.floor(x)
const noise = (n: number) => fract(Math.sin(n * 127.1) * 43758.5453)

/** A voice, from the clock alone, so every render draws the same bars: syllables inside words inside a phrase. */
export function speech(t: number) {
  const syllable = 0.5 + 0.5 * Math.sin(t * Math.PI * 2 * 4.6 + 2 * Math.sin(t * 1.7))
  const word = 0.55 + 0.45 * Math.sin(t * Math.PI * 2 * 0.9 + 1)
  return Math.min(1, 0.06 + syllable * word * (0.5 + 0.5 * noise(Math.floor(t * 30))))
}

/** Forty levels of a voice that was speaking up to time `t` (seconds). */
export function history(t: number, speaking = true) {
  return Array.from({ length: BARS }, (_, i) => (speaking ? speech(t - ((BARS - 1 - i) * FRAME_MS) / 1000) : 0))
}

type Props = {
  state: MeterState
  /** The voice is speaking: new levels push in. Silence pushes zeros, so the bars fall quiet from the right. */
  speaking?: boolean
  /** The timer, in seconds. */
  seconds?: number
  /** Fixed levels for a still; the meter then never animates them. */
  levels?: number[]
  className?: string
  style?: CSSProperties
} & Record<`data-${string}`, unknown>

export function Meter({ state, speaking = false, seconds = 0, levels, className, style, ...rest }: Props) {
  const speakingRef = useRef(speaking)
  speakingRef.current = speaking
  // While the meter fades out it keeps drawing the state it was in: the
  // bars and words never re-lay out under the fade
  const shownRef = useRef<MeterState>(state === 'hidden' ? 'recording' : state)
  if (state !== 'hidden') shownRef.current = state
  const shown = shownRef.current
  const [live, setLive] = useState<number[]>(() => levels ?? history(clock.now() / 1000, speaking))
  const [head, setHead] = useState(0)

  // The history: a new level every frame while recording, seeded with a
  // voice when the meter opens mid-sentence (the poster), zeros otherwise
  useEffect(() => {
    if (levels) return
    if (state !== 'recording') return
    setLive(history(clock.now() / 1000, speakingRef.current))
    const id = setInterval(() => {
      setLive((h) => [...h.slice(1), speakingRef.current ? speech(clock.now() / 1000) : 0])
    }, FRAME_MS)
    return () => clearInterval(id)
  }, [state, levels])

  // The scan: the glow head moves 1.1 bars a frame over the visible bars and
  // a little past them, as overlay.rs draws it
  useEffect(() => {
    if (state !== 'working') return
    const start = clock.now()
    setHead(0)
    const id = setInterval(() => setHead((((clock.now() - start) / FRAME_MS) * 1.1) % (BARS - WORK_START + 10)), FRAME_MS)
    return () => clearInterval(id)
  }, [state])

  const bars = Array.from({ length: BARS }, (_, i) => {
    if (shown === 'working') {
      const j = i - WORK_START
      const glow = j < 0 ? 0 : Math.max(0, 1 - Math.abs(j - head) / 5)
      return { h: BAR_W / 2 + glow * 7, fill: mix(IDLE, HIGH, glow), hidden: j < 0 }
    }
    const lvl = (levels ?? live)[i] ?? 0
    return { h: Math.max(lvl * MAX_BAR, BAR_W / 2), fill: lvl < 0.02 ? mix(IDLE, IDLE, 0) : mix(LOW, HIGH, lvl), hidden: false }
  })

  const timer = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`

  return (
    <div {...rest} className={`dd-meter${className ? ` ${className}` : ''}`} data-state={state} style={style}>
      <svg className="dd-meter__bars" width={BARS_W} height={H} viewBox={`0 0 ${BARS_W} ${H}`} aria-hidden>
        {bars.map((b, i) => (
          <rect key={i} className="dd-meter__bar" x={i * PITCH} y={H / 2 - b.h} width={BAR_W} height={2 * b.h} rx={BAR_W / 2} fill={b.fill} data-hidden={b.hidden || undefined} />
        ))}
      </svg>
      <svg className="dd-meter__mark" width={18} height={18} viewBox="0 0 18 18" aria-hidden>
        <g className="dd-meter__dot" data-on={shown === 'recording' || undefined}>
          <circle className="dd-meter__halo" cx={9} cy={9} r={8} fill={REC} />
          <circle cx={9} cy={9} r={4} fill={REC} />
        </g>
        <g className="dd-meter__pause" data-on={shown === 'paused' || undefined} fill={AMBER}>
          <rect x={9 - 3 - 1.2} y={9 - 5.2} width={2.4} height={10.4} rx={1.2} />
          <rect x={9 + 3 - 1.2} y={9 - 5.2} width={2.4} height={10.4} rx={1.2} />
        </g>
      </svg>
      {/* The timer changes as a clock does: the digits step, the slot holds */}
      <span className="dd-meter__text dd-meter__timer" data-on={shown === 'recording' || undefined} data-snap>
        {timer}
      </span>
      <span className="dd-meter__text dd-meter__paused" data-on={shown === 'paused' || undefined}>
        Paused
      </span>
      <span className="dd-meter__text dd-meter__label" data-on={shown === 'working' || undefined}>
        Transcribing
      </span>
    </div>
  )
}
