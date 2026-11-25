import { Eye } from 'lucide-react'
import { Field, Stage, Text, Toggle } from '../../kit'
import { AppIcon, WinWindow } from '../dictation-shared/win'
import './settings.css'

// The Settings window as a still: the whole window at true size, one gutter
// from the top and sides, off the bottom one gutter below its card, so the
// footer's buttons are out of frame rather than sliced and no empty band
// sits under the card. Gold: close-up (the real
// chrome says this is a crop of a real window; the subject row is whole).
//
// Real strings: the title, Settings and its line, the key field, the help
// line, the one setting.

export default function DictationSettings() {
  return (
    <Stage width={640} height={356} theme="dark" initial={{}} className="dd ds">
      {() => (
        <WinWindow title="Deepgram Dictation" caption="close" className="ds-win">
          <div className="ds-head">
            <AppIcon size={52} />
            <div className="ds-head__words">
              <Text className="ds-title">Settings</Text>
              <Text className="ds-sub">Press Alt+M in any app, speak, then press Alt+M again.</Text>
            </div>
          </div>
          <div className="ds-card">
            <Field className="ds-field" label="Deepgram API key" value="••••••••••••••••••••••••••••••••••••••" password trailing={<Eye size={16} strokeWidth={1.75} />} />
            <div className="ds-help">
              Get a key at <span className="ds-link">console.deepgram.com</span>, under API Keys.
            </div>
            <div className="ds-rule" />
            <div className="ds-row">
              <Text className="ds-setting">Start when I sign in to Windows</Text>
              <Toggle on />
            </div>
          </div>
        </WinWindow>
      )}
    </Stage>
  )
}
