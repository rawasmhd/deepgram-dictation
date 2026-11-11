import { Check } from 'lucide-react'
import { Stage } from '../../kit'
import { Taskbar } from '../dictation-shared/win'
import './tray.css'

// The tray menu as a still: the Windows 11 menu where the product puts it,
// above the tray at the bottom right of the screen. The menu is cloned from
// assets/tray-menu.png (its width, row height, inset and separators), so it
// is drawn here rather than from the kit's Menu. Every row is a real word,
// so the menu is whole; the taskbar runs off three edges.

const ITEMS: { label: string; key?: string; disabled?: boolean; checked?: boolean; sep?: boolean }[] = [
  { label: 'Deepgram Dictation', disabled: true },
  { label: 'Ready. Press Alt+M to dictate.', disabled: true },
  { label: 'Start dictation', key: 'Alt+M', sep: true },
  { label: 'Settings...', sep: true },
  { label: 'Custom words...' },
  { label: 'Start at login', checked: true },
  { label: 'Open log file' },
  { label: 'Quit', key: 'Ctrl+Alt+Q', sep: true },
]

export default function DictationTray() {
  return (
    <Stage width={560} height={320} theme="dark" initial={{}} className="dd dt">
      {() => (
        <>
          <Taskbar />
          <div className="dt-menu" role="menu">
            {ITEMS.map((item) => (
              <div key={item.label} className="dt-group" data-sep={item.sep || undefined}>
                <div className="dt-item" data-disabled={item.disabled || undefined}>
                  {item.checked && <Check className="dt-check" size={14} strokeWidth={2} />}
                  <span className="dt-label">{item.label}</span>
                  {item.key && <span className="dt-key">{item.key}</span>}
                </div>
              </div>
            ))}
          </div>
        </>
      )}
    </Stage>
  )
}
