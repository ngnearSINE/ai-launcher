import { listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { api } from './api'
import { locale, resolveLang, t } from './i18n.svelte'
import type { Installation, Instance, Settings, Snapshot } from './types'

export type View = 'home' | 'settings'

interface UiState {
  snap: Snapshot | null
  icons: Record<string, string>
  label: string
  view: View
  settingsTab: string
  /** Instance whose edit panel is open. */
  drawer: string | null
  /** Instance shown in the main pane; null = overview. */
  selected: string | null
  search: string
  /** Sidebar shrunk to an icon rail (Ctrl+B, or automatically in a narrow window). */
  railPinned: boolean
  narrow: boolean
  palette: boolean
  help: boolean
  /** Ctrl/Alt held long enough to show 1–9 badges on the first rows. */
  numbers: boolean
  /** Launch requested, waiting for the process to show up (spinning status ring). */
  launching: Record<string, number>
  /** Deleted but still undoable: hidden everywhere until the undo window closes. */
  pendingDelete: string[]
  dark: boolean
  now: number
}

export const ui: UiState = $state({
  snap: null,
  icons: {},
  label: 'main',
  view: 'home',
  settingsTab: 'general',
  drawer: null,
  selected: null,
  search: '',
  railPinned: localStorage.getItem('rail') === '1',
  narrow: false,
  palette: false,
  help: false,
  numbers: false,
  launching: {},
  pendingDelete: [],
  dark: false,
  now: Date.now(),
})

export function settings(): Settings {
  return ui.snap!.settings
}

export function installationOf(inst: Instance): Installation | undefined {
  return ui.snap?.installations.find((i) => i.id === inst.installationId)
}

export function instanceById(id: string | null): Instance | undefined {
  return id ? ui.snap?.instances.find((i) => i.id === id) : undefined
}

/** "Claude", "Gemini" … shown under an alias when it differs. */
export function appName(inst: Instance): string {
  return installationOf(inst)?.name ?? ''
}

/** Worst of the 5-hour and weekly use of the organisations the instance shows. */
export function headroom(inst: Instance): number | null {
  const orgs = visibleOrgs(inst)
  const vals = orgs.flatMap((o) => o.quotas.filter((q) => q.kind === 'fiveHour' || q.kind === 'weekly').map((q) => q.usedPercent))
  return vals.length ? Math.max(...vals) : null
}

export function visibleOrgs(inst: Instance) {
  const orgs = inst.usage?.report?.orgs ?? []
  const shown = inst.shownOrgs ?? []
  const list = shown.length ? orgs.filter((o) => shown.includes(o.orgUuid)) : orgs
  return list.length ? list : orgs
}

/** Every instance the user can see (hidden apps and undoable deletes left out), in the
 *  chosen sort order. The sidebar, the floating launcher and the palette all start here. */
export function visibleInstances(opts: { forQuick?: boolean; forFloat?: boolean } = {}): Instance[] {
  const s = ui.snap
  if (!s) return []
  let list = s.instances.filter((i) => {
    if (ui.pendingDelete.includes(i.id)) return false
    if (opts.forQuick && i.advanced.hiddenInQuick) return false
    if (opts.forFloat && i.advanced.hiddenInFloat) return false
    return !installationOf(i)?.hidden
  })
  const sort = s.settings.sort
  if (sort === 'name') list = [...list].sort((a, b) => a.alias.localeCompare(b.alias))
  else if (sort === 'recent') list = [...list].sort((a, b) => (b.lastLaunchedAt ?? '').localeCompare(a.lastLaunchedAt ?? ''))
  else if (sort === 'headroom') list = [...list].sort((a, b) => (headroom(a) ?? 101) - (headroom(b) ?? 101))
  return list
}

// ---- toast (one at a time, the newest replaces the last) -------------------------------------

export interface ToastAction {
  label: string
  run: () => unknown
}
export interface Toast {
  id: number
  text: string
  kind: 'info' | 'error' | 'ok' | 'warn'
  action?: ToastAction
  /** ms; null keeps it until dismissed. */
  duration: number | null
}
export const toastState: { current: Toast | null; paused: boolean } = $state({ current: null, paused: false })
let toastSeq = 0
let toastTimer: ReturnType<typeof setTimeout> | undefined
let toastLeft = 0
let toastStarted = 0

function armToast(ms: number) {
  clearTimeout(toastTimer)
  toastStarted = Date.now()
  toastLeft = ms
  const id = toastState.current?.id
  toastTimer = setTimeout(() => {
    if (toastState.current?.id === id) toastState.current = null
  }, ms)
}

export function toast(text: string, kind: Toast['kind'] = 'info', opts: { action?: ToastAction; duration?: number | null } = {}) {
  const duration = opts.duration !== undefined ? opts.duration : opts.action ? 6000 : kind === 'error' ? 3200 : 2200
  toastState.current = { id: ++toastSeq, text, kind, action: opts.action, duration }
  toastState.paused = false
  clearTimeout(toastTimer)
  if (duration !== null) armToast(duration)
}

/** Hovering a toast stops its clock; leaving restarts it with what was left. */
export function pauseToast(on: boolean) {
  const cur = toastState.current
  if (!cur || cur.duration === null || toastState.paused === on) return
  toastState.paused = on
  if (on) {
    clearTimeout(toastTimer)
    toastLeft = Math.max(800, toastLeft - (Date.now() - toastStarted))
  } else armToast(toastLeft)
}

export function dismissToast() {
  clearTimeout(toastTimer)
  toastState.current = null
}

/** Shows a backend error code as a sentence. */
export function errorText(e: unknown): string {
  const code = String(e)
  const map: Record<string, string> = {
    'instance-missing': t('인스턴스를 찾을 수 없어요'),
    'installation-missing': t('앱이 설치돼 있지 않거나 찾을 수 없어요'),
    'browser-missing': t('웹앱을 열 브라우저가 없어요'),
    'instance-running': t('실행 중에는 데이터를 지울 수 없어요. 먼저 종료하세요'),
    'has-instances': t('이 앱의 인스턴스가 남아 있어요'),
    unrecognized: t('실행 파일을 인식하지 못했어요'),
    'not-a-file': t('파일을 찾을 수 없어요'),
    'url-invalid': t('주소는 https:// 로 시작해야 해요'),
    'name-missing': t('이름을 입력하세요'),
    'no-claude': t('Claude 인스턴스가 없어요'),
  }
  return map[code] ?? code
}

export async function run<T>(p: Promise<T>, ok?: string): Promise<T | undefined> {
  try {
    const r = await p
    if (ok) toast(ok, 'ok')
    return r
  } catch (e) {
    toast(errorText(e), 'error')
    return undefined
  }
}

export interface ConfirmReq {
  title: string
  message: string
  confirm: string
  danger?: boolean
  checkbox?: { label: string; value: boolean }
  resolve: (r: { ok: boolean; checked: boolean }) => void
}
export const dialogs: { confirm: ConfirmReq | null; newInstance: boolean | string; addApp: boolean } = $state({
  confirm: null,
  newInstance: false,
  addApp: false,
})
export function confirmBox(o: Omit<ConfirmReq, 'resolve'>): Promise<{ ok: boolean; checked: boolean }> {
  return new Promise((resolve) => (dialogs.confirm = { ...o, resolve }))
}

export interface MenuItem {
  label?: string
  icon?: string
  hint?: string
  danger?: boolean
  disabled?: boolean
  sep?: boolean
  checked?: boolean
  action?: () => unknown
}
export const menu: { open: boolean; x: number; y: number; items: MenuItem[] } = $state({ open: false, x: 0, y: 0, items: [] })
export function openMenu(e: MouseEvent | { clientX: number; clientY: number }, items: MenuItem[]) {
  menu.items = items
  menu.x = e.clientX
  menu.y = e.clientY
  menu.open = true
}

// ---- theme -----------------------------------------------------------------------------------

export const THEMES = ['system', 'light', 'dark', 'claude', 'midnight', 'black'] as const
export type ThemeId = (typeof THEMES)[number]
/** Dark variants carry their own accent where the look depends on it. */
const THEME_ACCENT: Partial<Record<ThemeId, string>> = { claude: '#d97757' }

let syncedTheme: 'dark' | 'light' | null | undefined = undefined
function applyTheme() {
  const s = ui.snap?.settings
  if (!s) return
  const root = document.documentElement
  const theme = s.theme as ThemeId
  const variant = theme === 'claude' || theme === 'midnight' || theme === 'black' ? theme : ''
  const dark = !!variant || theme === 'dark' || (theme === 'system' && matchMedia('(prefers-color-scheme: dark)').matches)
  const windowTheme = theme === 'system' ? null : dark ? 'dark' : 'light'
  if (windowTheme !== syncedTheme) {
    // The window material (Mica / Acrylic) follows the window's own theme; match the app's,
    // or a light app sits on a dark-tinted backdrop when Windows is dark (and vice versa).
    syncedTheme = windowTheme
    getCurrentWindow().setTheme(windowTheme).catch(() => {})
  }
  ui.dark = dark
  root.dataset.theme = dark ? 'dark' : 'light'
  if (variant) root.dataset.variant = variant
  else delete root.dataset.variant
  root.dataset.glass = s.glass ? 'on' : 'off'
  root.dataset.density = s.density
  root.dataset.motion = s.reduceMotion ? 'reduce' : 'normal'
  root.style.setProperty('--accent', THEME_ACCENT[theme] ?? s.accent)
  root.style.setProperty('--glass-k', s.glass ? String(s.glassStrength / 100) : '0')
  locale.lang = resolveLang(s.language)
  document.documentElement.lang = locale.lang
}

// ---- boot ------------------------------------------------------------------------------------

export async function boot() {
  ui.label = getCurrentWindow().label
  ui.snap = await api.state()
  applyTheme()
  await listen<Snapshot>('state', (e) => {
    const iconsStale = (ui.snap?.installations.length ?? 0) !== e.payload.installations.length || e.payload.instances.some((i) => i.running && !ui.icons[i.installationId])
    ui.snap = e.payload
    // A launch is done once the process shows up.
    for (const i of e.payload.instances) if (i.running && ui.launching[i.id]) delete ui.launching[i.id]
    if (ui.selected && !e.payload.instances.some((i) => i.id === ui.selected)) ui.selected = null
    applyTheme()
    if (iconsStale) loadIcons()
  })
  await listen<string>('navigate', (e) => {
    if (e.payload === 'settings') ui.view = 'settings'
  })
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme)
  // Countdowns are in minutes, so a coarse clock is enough (and keeps every window idle).
  setInterval(() => (ui.now = Date.now()), 15_000)
  setInterval(() => {
    // Give up on the spinner if the app never appeared (it may have refused to start).
    const now = Date.now()
    for (const [id, at] of Object.entries(ui.launching)) if (now - at > 20_000) delete ui.launching[id]
  }, 2000)
  loadIcons()
}

let iconsLoading = false
async function loadIcons() {
  if (iconsLoading) return
  iconsLoading = true
  try {
    ui.icons = await api.appIcons()
  } catch {
    /* icons are optional */
  } finally {
    iconsLoading = false
  }
}
