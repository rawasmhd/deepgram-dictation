import { Search } from 'lucide-react'
import { Bar, Stage, Text, type Act } from '../../kit'
import { WinKeys, WinWindow } from '../dictation-shared/win'
import { Meter, type MeterState } from '../dictation-shared/meter'
import './hero.css'

// The README hero: one dictation, as it feels to use. A notes window on a
// dark desktop; the meter floats above the taskbar line. Beats: Alt+M opens
// the meter; the voice runs and the timer counts; the first phrase lands in
// the note while the voice goes on (live paste); Alt+M again; Transcribing;
// the last phrase lands and the meter leaves. The note holds. Ctrl+Alt+Z,
// the app's own undo, takes the dictation back as one thing, so the loop
// has a seen cause.
//
// Gold: dark-hero (the window a step lighter than the page, one lit colour,
// the list of filler rows) with hero-demo's corner bleed. No cursor: this is
// a keyboard story, so the keycaps carry it.
//
// Real strings: the window title, the note's heading, its first line, the
// two dictated phrases, the timer, Transcribing. The sidebar is bars.

type S = {
  meter: MeterState
  speaking: boolean
  seconds: number
  text: string
  keys: string[]
  keysShow: boolean
  keysDown: boolean
}

const P1 = 'Send the revised quote to Dr. Okafor before Thursday.'
const P2 = 'Ask Lina to confirm the delivery window.'

const initial: S = { meter: 'hidden', speaking: false, seconds: 0, text: '', keys: [], keysShow: false, keysDown: false }
const poster: S = { ...initial, meter: 'recording', speaking: true, seconds: 3, text: P1 }

// A keystroke: the caps arrive, go down, come up (the app reacts here), and fade later
async function press(a: Act<S>, keys: string[]) {
  a.put({ keys, keysShow: true, keysDown: false })
  await a.wait(180)
  a.put({ keysDown: true })
  await a.wait(120)
  a.put({ keysDown: false })
  void a.wait(600).then(() => a.put({ keysShow: false }))
}

async function script(a: Act<S>) {
  await a.wait(900)
  await press(a, ['Alt', 'M'])
  a.mark('open')
  a.put({ meter: 'recording', seconds: 0 })
  await a.wait(500)
  a.put({ speaking: true })
  await a.wait(500)
  a.put({ seconds: 1 })
  await a.wait(1000)
  a.put({ seconds: 2 })
  await a.wait(1000)
  a.put({ seconds: 3 })
  await a.wait(400)
  a.put({ speaking: false })
  await a.wait(300)
  // The first phrase is final: pasted while the voice goes on
  a.mark('phrase1')
  a.put({ text: P1 })
  await a.wait(300)
  a.put({ seconds: 4 })
  await a.wait(100)
  a.put({ speaking: true })
  await a.wait(900)
  a.put({ seconds: 5 })
  await a.wait(1000)
  a.put({ seconds: 6 })
  await a.wait(700)
  a.put({ speaking: false })
  await a.wait(300)
  a.put({ seconds: 7 })
  await press(a, ['Alt', 'M'])
  a.mark('working')
  a.put({ meter: 'working' })
  await a.wait(1300)
  // The rest is pasted and the meter goes
  a.mark('phrase2')
  a.put({ text: `${P1} ${P2}`, meter: 'hidden' })
  await a.wait(2200)
  // Undo takes the whole dictation back: the loop's reset, with its cause
  await press(a, ['Ctrl', 'Alt', 'Z'])
  a.mark('undo')
  a.put({ text: '' })
  await a.wait(1400)
  // The meter is hidden and the caps have faded: back to the first frame unseen
  await a.snap({ seconds: 0, keys: [] })
}

const NOTES: [number, number][] = [
  [96, 52],
  [120, 48],
  [80, 56],
  [110, 44],
  [70, 50],
]

export default function DictationHero() {
  return (
    <Stage<S> width={720} height={340} theme="dark" initial={initial} poster={poster} script={script} className="dd dh">
      {(s) => (
        <>
          <WinWindow title="Notes" icon={<Bar w={16} h={16} r={4} tone={4} />} className="dh-win">
            <aside className="dh-side">
              <div className="dh-search">
                <Search size={14} strokeWidth={1.75} />
                <Bar w={60} h={6} r={3} tone={2} />
              </div>
              {NOTES.map(([title, date], i) => (
                <div key={i} className="dh-note" data-active={i === 0 || undefined}>
                  <Bar w={title} h={8} r={4} tone={i === 0 ? 4 : 3} />
                  <Bar w={date} h={6} r={3} tone={2} />
                </div>
              ))}
            </aside>
            <main className="dh-editor">
              <Text className="dh-heading">Site visit</Text>
              <Bar w={112} h={6} r={3} tone={2} className="dh-date" />
              <p className="dh-para">Met the contractor at nine for the roof inspection.</p>
              {/* The dictation is pasted, so it arrives as one operation */}
              <p className="dh-para dh-para--live">
                <span data-snap>{s.text}</span>
                <span className="dh-caret" />
              </p>
            </main>
          </WinWindow>
          <Meter className="dh-meter" state={s.meter} speaking={s.speaking} seconds={s.seconds} />
          <WinKeys keys={s.keys} show={s.keysShow} down={s.keysDown} />
        </>
      )}
    </Stage>
  )
}
