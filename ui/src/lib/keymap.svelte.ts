// In-window keyboard shortcuts: one table that the key handler, the shortcut help dialog,
// menus and the settings page all read. Ids are stable so a user's remapping (settings.keymap,
// "" = unassigned) survives updates. Keys are matched by physical key (e.code), so they work
// with a Korean IME too. Global hotkeys (work outside the window) are separate settings.
import { t } from './i18n.svelte'
import { ui } from './state.svelte'

export type Section = 'general' | 'instances' | 'layout'

export interface Binding {
  id: string
  section: Section
  label: () => string
  keys: string
  /** Shown but not remappable (ranges like Ctrl+1…9, plain arrows). */
  fixed?: boolean
}

export const BINDINGS: Binding[] = [
  { id: 'palette', section: 'general', label: () => t('명령 팔레트'), keys: 'Ctrl+K' },
  { id: 'newInstance', section: 'general', label: () => t('새 인스턴스'), keys: 'Ctrl+N' },
  { id: 'settings', section: 'general', label: () => t('설정'), keys: 'Ctrl+,' },
  { id: 'help', section: 'general', label: () => t('단축키 보기'), keys: 'Shift+/' },
  { id: 'theme', section: 'general', label: () => t('테마 바꾸기'), keys: 'Ctrl+Alt+T' },
  { id: 'undo', section: 'general', label: () => t('삭제 되돌리기'), keys: 'Ctrl+Z' },
  { id: 'launch', section: 'instances', label: () => t('선택한 인스턴스 실행·전환'), keys: 'Enter' },
  { id: 'launchNth', section: 'instances', label: () => t('n번째 인스턴스 실행'), keys: 'Ctrl+1…9', fixed: true },
  { id: 'next', section: 'instances', label: () => t('다음 인스턴스'), keys: 'Ctrl+]' },
  { id: 'prev', section: 'instances', label: () => t('이전 인스턴스'), keys: 'Ctrl+[' },
  { id: 'move', section: 'instances', label: () => t('목록에서 위아래로'), keys: 'Up / Down', fixed: true },
  { id: 'edit', section: 'instances', label: () => t('편집'), keys: 'F2' },
  { id: 'pin', section: 'instances', label: () => t('위에 고정·해제'), keys: 'Ctrl+Shift+P' },
  { id: 'restart', section: 'instances', label: () => t('다시 시작'), keys: '' },
  { id: 'quit', section: 'instances', label: () => t('강제 종료'), keys: '' },
  { id: 'delete', section: 'instances', label: () => t('삭제'), keys: 'Delete' },
  { id: 'sidebar', section: 'layout', label: () => t('사이드바 접기·펴기'), keys: 'Ctrl+B' },
  { id: 'overview', section: 'layout', label: () => t('개요로'), keys: 'Ctrl+0' },
  { id: 'back', section: 'layout', label: () => t('닫기·뒤로'), keys: 'Escape', fixed: true },
]

export const SECTIONS: { id: Section; label: () => string }[] = [
  { id: 'general', label: () => t('일반') },
  { id: 'instances', label: () => t('인스턴스') },
  { id: 'layout', label: () => t('화면') },
]

/** Current keys of a binding, with the user's override. */
export function keysOf(id: string): string {
  const b = BINDINGS.find((x) => x.id === id)
  const custom = ui.snap?.settings.keymap?.[id]
  return custom !== undefined && !b?.fixed ? custom : (b?.keys ?? '')
}

const CODE_NAMES: Record<string, string> = {
  Slash: '/',
  Comma: ',',
  Period: '.',
  BracketLeft: '[',
  BracketRight: ']',
  Backquote: '`',
  Minus: '-',
  Equal: '=',
  Semicolon: ';',
  Quote: "'",
  Backslash: '\\',
  Space: 'Space',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  NumpadEnter: 'Enter',
}

/** "Ctrl+Shift+P" for a key event (physical key), or null for a bare modifier. */
export function comboOf(e: KeyboardEvent): string | null {
  if (['ControlLeft', 'ControlRight', 'ShiftLeft', 'ShiftRight', 'AltLeft', 'AltRight', 'MetaLeft', 'MetaRight'].includes(e.code)) return null
  const key = CODE_NAMES[e.code] ?? (e.code.startsWith('Key') ? e.code.slice(3) : e.code.startsWith('Digit') ? e.code.slice(5) : e.code.startsWith('Numpad') ? e.code.slice(6) : e.code)
  const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super'].filter(Boolean)
  return [...mods, key].join('+')
}

export function matches(e: KeyboardEvent, id: string): boolean {
  const keys = keysOf(id)
  if (!keys) return false
  const c = comboOf(e)
  return !!c && keys.split(' / ').some((k) => k === c)
}

/** Key caps for display: "Shift+/" shows as "Shift" "?". */
export function caps(combo: string): string[] {
  if (!combo) return []
  return combo
    .split('+')
    .map((p, _i, all) => (p === '/' && all.includes('Shift') ? '?' : p))
    .filter((p, _i, all) => !(p === 'Shift' && all.includes('?')))
}

/** Typing into a field: only bindings with a modifier (or Escape) apply. */
export function typing(e: KeyboardEvent): boolean {
  const el = e.target as HTMLElement | null
  return !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable)
}
