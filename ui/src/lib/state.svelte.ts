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
  drawer: string | null
  search: string
  filter: string
  layout: 'grid' | 'list'
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
  search: '',
  filter: 'all',
  layout: (localStorage.getItem('layout') as 'grid' | 'list') || 'grid',
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
  const pick = inst.advanced.usageOrgs
  return pick.length ? orgs.filter((o) => pick.includes(o.orgUuid)) : orgs
}

/** Instances after search / filter / sort, for the main grid and the quick panel. */
export function visibleInstances(opts: { forQuick?: boolean } = {}): Instance[] {
  const s = ui.snap
  if (!s) return []
  const q = ui.search.trim().toLowerCase()
  let list = s.instances.filter((i) => {
    if (opts.forQuick && i.advanced.hiddenInQuick) return false
    const install = installationOf(i)
    if (install?.hidden) return false
    if (q) {
      const email = i.usage?.report?.email ?? ''
      const hay = `${i.alias} ${install?.name ?? ''} ${email} ${i.note}`.toLowerCase()
      if (!hay.includes(q)) return false
    }
    if (opts.forQuick) return true
    const f = ui.filter
    if (f === 'running') return i.running
    if (f === 'favorite') return i.favorite
    if (f.startsWith('app:')) return i.installationId === f.slice(4)
    if (f.startsWith('group:')) return i.groupId === f.slice(6)
    return true
  })
  const sort = s.settings.sort
  if (sort === 'name') list = [...list].sort((a, b) => a.alias.localeCompare(b.alias))
  else if (sort === 'recent') list = [...list].sort((a, b) => (b.lastLaunchedAt ?? '').localeCompare(a.lastLaunchedAt ?? ''))
  else if (sort === 'headroom') list = [...list].sort((a, b) => (headroom(a) ?? 101) - (headroom(b) ?? 101))
  if (opts.forQuick) list = [...list].sort((a, b) => Number(b.favorite) - Number(a.favorite))
  return list
}

// ---- toasts / confirm / context menu -------------------------------------------------------

export interface Toast {
  id: number
  text: string
  kind: 'info' | 'error' | 'ok'
}
export const toasts: Toast[] = $state([])
let toastSeq = 0
export function toast(text: string, kind: Toast['kind'] = 'info') {
  const id = ++toastSeq
  toasts.push({ id, text, kind })
  setTimeout(() => {
    const i = toasts.findIndex((x) => x.id === id)
    if (i >= 0) toasts.splice(i, 1)
  }, kind === 'error' ? 6000 : 3200)
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
  action?: () => unknown
}
export const menu: { open: boolean; x: number; y: number; items: MenuItem[] } = $state({ open: false, x: 0, y: 0, items: [] })
export function openMenu(e: MouseEvent | { clientX: number; clientY: number }, items: MenuItem[]) {
  menu.items = items
  menu.x = e.clientX
  menu.y = e.clientY
  menu.open = true
}

// ---- boot ------------------------------------------------------------------------------------

function applyTheme() {
  const s = ui.snap?.settings
  if (!s) return
  const root = document.documentElement
  const dark = s.theme === 'dark' || (s.theme === 'system' && matchMedia('(prefers-color-scheme: dark)').matches)
  ui.dark = dark
  root.dataset.theme = dark ? 'dark' : 'light'
  root.dataset.glass = s.glass ? 'on' : 'off'
  root.dataset.density = s.density
  root.dataset.motion = s.reduceMotion ? 'reduce' : 'normal'
  root.style.setProperty('--accent', s.accent)
  root.style.setProperty('--glass-k', s.glass ? String(s.glassStrength / 100) : '0')
  locale.lang = resolveLang(s.language)
  document.documentElement.lang = locale.lang
}

export async function boot() {
  ui.label = getCurrentWindow().label
  ui.snap = await api.state()
  applyTheme()
  await listen<Snapshot>('state', (e) => {
    const iconsStale = (ui.snap?.installations.length ?? 0) !== e.payload.installations.length || e.payload.instances.some((i) => i.running && !ui.icons[i.installationId])
    ui.snap = e.payload
    applyTheme()
    if (iconsStale) loadIcons()
  })
  await listen<string>('navigate', (e) => {
    if (e.payload === 'settings') ui.view = 'settings'
  })
  matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme)
  setInterval(() => (ui.now = Date.now()), 30_000)
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
