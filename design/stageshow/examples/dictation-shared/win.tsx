import { useId, type CSSProperties, type HTMLAttributes, type ReactNode } from 'react'
import { ChevronUp, Wifi, Volume2, BatteryMedium } from 'lucide-react'
import { Bar } from '../../kit'
import './look.css'
import './win.css'

// Windows 11 chrome and the app's own marks, drawn from the real app's
// screenshots (assets/settings.png, assets/tray-menu.png) and from
// rust/src/art.rs. The kit's Window draws macOS lights, so a Windows app
// gets its real title bar here: a 16px icon, a 12px title, caption buttons
// 46px wide on the right.

/** The app icon from art.rs: a microphone and sound bars on a teal-to-blue tile. Under 24px, the simple one without bars. */
export function AppIcon({ size = 52, className, style }: { size?: number; className?: string; style?: CSSProperties }) {
  const id = useId()
  const grad = (
    <defs>
      <linearGradient id={id} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="#1fa88c" />
        <stop offset="1" stopColor="#3b9edb" />
      </linearGradient>
    </defs>
  )
  if (size < 24) {
    return (
      <svg className={className} style={style} width={size} height={size} viewBox="0 0 16 16" aria-hidden>
        {grad}
        <rect x="0.5" y="0.5" width="15" height="15" rx="3.5" fill={`url(#${id})`} />
        <rect x="6" y="2.5" width="4" height="7" rx="2" fill="#fff" />
        <path d="M4.3 7.6a3.7 3.7 0 0 0 7.4 0M8 11.4v2" fill="none" stroke="#fff" strokeWidth="1.3" strokeLinecap="round" />
      </svg>
    )
  }
  return (
    <svg className={className} style={style} width={size} height={size} viewBox="0 0 256 256" aria-hidden>
      {grad}
      <rect x="8" y="8" width="240" height="240" rx="56" fill={`url(#${id})`} />
      <rect x="108" y="60" width="40" height="80" rx="20" fill="#fff" />
      <path d="M88 116v6a40 40 0 0 0 80 0v-6M128 162v22M108 184h40" fill="none" stroke="#fff" strokeWidth="12" strokeLinecap="round" />
      <path d="M68 96v56M50 112v24M188 96v56M206 112v24" fill="none" stroke="#fff" strokeWidth="12" strokeLinecap="round" opacity="0.72" />
    </svg>
  )
}

/** The tray icon from art.rs: the tile in one colour with the microphone cut out. */
export function TrayIcon({ size = 16, ...rest }: { size?: number } & Record<`data-${string}`, unknown>) {
  const id = useId()
  return (
    <svg {...rest} width={size} height={size} viewBox="0 0 16 16" aria-hidden>
      <defs>
        <mask id={id}>
          <rect width="16" height="16" fill="#fff" />
          <rect x="6" y="2.5" width="4" height="7" rx="2" fill="#000" />
          <path d="M4.3 7.6a3.7 3.7 0 0 0 7.4 0M8 11.4v2" fill="none" stroke="#000" strokeWidth="1.3" strokeLinecap="round" />
        </mask>
      </defs>
      <rect x="0.5" y="0.5" width="15" height="15" rx="3.5" fill="currentColor" mask={`url(#${id})`} />
    </svg>
  )
}

function Caption({ kind }: { kind: 'min' | 'max' | 'close' }) {
  return (
    <span className="dd-win__cap" data-kind={kind}>
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1">
        {kind === 'min' && <path d="M0 5h10" />}
        {kind === 'max' && <rect x="0.5" y="0.5" width="9" height="9" rx="1.5" />}
        {kind === 'close' && <path d="M0.5 0.5l9 9M9.5 0.5l-9 9" />}
      </svg>
    </span>
  )
}

/** A Windows 11 window: the title bar with the app icon, the title and the caption buttons. */
export function WinWindow({ title, icon, caption = 'all', className, children, ...rest }: HTMLAttributes<HTMLDivElement> & { title: string; icon?: ReactNode; caption?: 'all' | 'close' }) {
  return (
    <div {...rest} className={`dd-win${className ? ` ${className}` : ''}`}>
      <div className="dd-win__bar">
        <span className="dd-win__icon">{icon ?? <AppIcon size={16} />}</span>
        <span className="dd-win__title">{title}</span>
        <span className="dd-win__caption">
          {caption === 'all' && (
            <>
              <Caption kind="min" />
              <Caption kind="max" />
            </>
          )}
          <Caption kind="close" />
        </span>
      </div>
      <div className="dd-win__body">{children}</div>
    </div>
  )
}

/** The right end of a Windows 11 taskbar: the tray with the app's icon in it. The centred app tiles are far to the left of this crop. */
export function Taskbar({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div {...rest} className={`dd-taskbar${className ? ` ${className}` : ''}`}>
      <div className="dd-taskbar__tray">
        <ChevronUp size={16} strokeWidth={1.75} />
        <TrayIcon size={16} data-target="tray" />
        <Wifi size={16} strokeWidth={1.75} />
        <Volume2 size={16} strokeWidth={1.75} />
        <BatteryMedium size={16} strokeWidth={1.75} />
        <span className="dd-taskbar__clock">
          <Bar w={30} h={6} tone={3} />
          <Bar w={44} h={6} tone={2} />
        </span>
      </div>
    </div>
  )
}

/** Keystrokes as keycaps with Windows names (Alt, Ctrl), where the kit's caps would print Mac symbols. */
export function WinKeys({ keys, show, down }: { keys: string[]; show: boolean; down: boolean }) {
  return (
    <div className="dd-keys" data-show={show || undefined} aria-hidden>
      {keys.map((k, i) => (
        <kbd key={i} className="dd-key" data-down={down || undefined}>
          {k}
        </kbd>
      ))}
    </div>
  )
}
