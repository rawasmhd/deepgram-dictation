import { Stage } from '../../kit'
import { Taskbar } from '../dictation-shared/win'
import { Meter, history } from '../dictation-shared/meter'
import './paused.css'

// The meter while another window has focus: a strip of the screen's bottom,
// the taskbar running off three edges and the meter 12px above it, where
// overlay.rs puts it. The bars hold the voice as it was when the focus left.

const LEVELS = history(2.0)

export default function DictationPaused() {
  return (
    <Stage width={460} height={140} theme="dark" initial={{}} className="dd dp">
      {() => (
        <>
          <Taskbar />
          <Meter className="dp-meter" state="paused" levels={LEVELS} />
        </>
      )}
    </Stage>
  )
}
