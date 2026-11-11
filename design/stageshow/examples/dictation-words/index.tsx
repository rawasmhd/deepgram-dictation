import { Bar, Button, Field, Stage, Text } from '../../kit'
import { AppIcon, WinWindow } from '../dictation-shared/win'
import './words.css'

// The Custom words window as a still, framed like Settings: the window whole
// across, off the bottom through its list. The words are the subject, so the
// first rows are real and the rows the crop takes are bars. Remove, the
// count and the footer are below the frame.

const WORDS = ['Deepgram', 'GitHub Actions', 'Kubernetes', 'Dr. Okafor']

export default function DictationWords() {
  return (
    <Stage width={640} height={420} theme="dark" initial={{}} className="dd dw">
      {() => (
        <WinWindow title="Custom words - Deepgram Dictation" caption="close" className="dw-win">
          <div className="dw-head">
            <AppIcon size={52} />
            <div className="dw-head__words">
              <Text className="dw-title">Custom words</Text>
              <Text className="dw-sub">Deepgram listens for these names and words.</Text>
            </div>
          </div>
          <div className="dw-card">
            <Text className="dw-label">Add a word or phrase</Text>
            <div className="dw-add">
              <Field className="dw-field" value="" focused />
              <Button size="large">Add</Button>
            </div>
            <div className="dw-list">
              {WORDS.map((w) => (
                <div key={w} className="dw-word">
                  <Text className="dw-word__text">{w}</Text>
                </div>
              ))}
              {[72, 96].map((w, i) => (
                <div key={i} className="dw-word">
                  <Bar w={w} h={8} r={4} tone={3} />
                </div>
              ))}
            </div>
          </div>
        </WinWindow>
      )}
    </Stage>
  )
}
