// Usage model shared by cards, the drawer, the quick panel and the floating launcher.
// Same rules as the team's AI Usage widget: values show what is left by default, a pace tick
// marks where usage would be if spread evenly, and "pace warning" only fires when the window
// would run out well before it resets and half or less is left.
import type { Instance, OrgUsage, Quota, Settings, UsageError } from './types'
import { installationOf, ui, visibleOrgs } from './state.svelte'
import { t } from './i18n.svelte'
import { until } from './format'

const HOUR = 3600e3
const LEN: Record<string, number> = { fiveHour: 5 * HOUR, weekly: 7 * 24 * HOUR, scoped: 7 * 24 * HOUR }

export interface Win {
  q: Quota
  left: number
  used: number
  /** Value shown (left or used, per settings). */
  shown: number
  /** Where `shown` would be at an even pace (0–100), or null. */
  paceAt: number | null
  resetMs: number | null
  /** At the current rate, ms until nothing is left (null when unknown or never). */
  emptyMs: number | null
  runsOut: boolean
  paceWarn: boolean
}

export function win(q: Quota, s: Settings, now = Date.now()): Win {
  const used = Math.max(0, Math.min(100, q.usedPercent))
  const left = 100 - used
  const len = LEN[q.kind] ?? null
  const resetMs = q.resetsAt ? Math.max(0, Date.parse(q.resetsAt) - now) : null
  const elapsed = len && resetMs !== null ? Math.max(0.02, Math.min(1, 1 - resetMs / len)) : null
  const rate = elapsed && len ? used / (elapsed * len) : 0
  const toEmpty = elapsed === null ? null : rate > 0 ? left / rate : Infinity
  const runsOut = used > 0 && toEmpty !== null && resetMs !== null && toEmpty < resetMs
  const paceWarn = runsOut && len !== null && resetMs !== null && toEmpty !== null && resetMs - toEmpty >= len * 0.15 && left <= 50
  const showLeft = s.usageValue !== 'used'
  const expectedLeft = elapsed === null ? null : (1 - elapsed) * 100
  return {
    q,
    left,
    used,
    shown: showLeft ? left : used,
    paceAt: expectedLeft === null || !s.usageShowPace ? null : showLeft ? expectedLeft : 100 - expectedLeft,
    resetMs,
    emptyMs: toEmpty !== null && Number.isFinite(toEmpty) ? toEmpty : null,
    runsOut,
    paceWarn,
  }
}

/** The line under a usage bar: the countdown first (what you act on), or — only under a pace
 *  warning (half or less left and running out well before the reset) — when it runs out. */
export function windowNote(w: Win, now = Date.now()): string {
  if (w.paceWarn && w.emptyMs !== null) return t('지금 속도면 {time} 뒤 바닥나요', { time: until(new Date(now + w.emptyMs).toISOString(), now) })
  if (w.q.resetsAt) return t('{time} 뒤 초기화', { time: until(w.q.resetsAt, now) })
  return ''
}

// ---- instance status (sidebar groups, status badges, palette) ---------------------------------

export type Status = 'needs' | 'crit' | 'warn' | 'running' | 'idle'

/** Needs the user (app missing / signed out) > running low > running > idle. */
export function statusOf(inst: Instance, s: Settings): Status {
  const install = installationOf(inst)
  if (!install?.present) return 'needs'
  if (install.hasUsage && inst.effective.usageEnabled && !inst.usage?.report) {
    const code = inst.usage?.error?.code
    if (code === 'notSignedIn' || code === 'sessionExpired' || code === 'accountUnknown' || code === 'decrypt') return 'needs'
  }
  const sev = install.hasUsage && inst.effective.usageEnabled ? severity(mainOrg(inst), s) : ''
  if (sev) return sev
  return inst.running ? 'running' : 'idle'
}

/** Why an instance needs attention, as one short sentence (null when it does not). */
export function needsText(inst: Instance): string | null {
  const install = installationOf(inst)
  if (!install?.present) return t('앱을 찾을 수 없어요')
  const code = inst.usage?.error?.code
  if (code === 'notSignedIn') return t('로그인이 필요해요')
  if (code === 'sessionExpired') return t('세션이 만료됐어요')
  if (code === 'accountUnknown') return t('계정을 확인할 수 없어요')
  if (code === 'decrypt') return t('로그인 정보를 읽지 못했어요')
  return null
}

/** A usage error as a sentence: signed-out / expired read as facts with a next step, not alarms. */
export function usageErrorText(e: UsageError | null | undefined): string {
  if (!e) return ''
  switch (e.code) {
    case 'notSignedIn':
      return t('로그인하면 사용량이 보여요')
    case 'sessionExpired':
      return t('세션이 만료됐어요 · 앱을 열면 갱신돼요')
    case 'accountUnknown':
      return t('이 프로필의 계정을 확인할 수 없어요')
    case 'decrypt':
      return t('로그인 정보를 읽지 못했어요')
    case 'rateLimited':
      return t('요청 제한 · {n}분 뒤 다시 시도', { n: Math.ceil(e.detail / 60) })
    case 'network':
      return t('네트워크 오류')
    default:
      return t('사용량을 불러오지 못했어요')
  }
}

/** The Claude instance with the most headroom (least of 5-hour/weekly used), like the Rust
 *  `launch_best_claude`. Instances without numbers count as half used. */
export function bestClaude(list: Instance[], s: Settings, except?: string): Instance | undefined {
  const score = (i: Instance) => {
    const { w5, ww } = windowsOf(mainOrg(i), s)
    const used = [w5, ww].filter(Boolean).map((w) => (w as Win).used)
    return used.length ? Math.max(...used) : 50
  }
  return list
    .filter((i) => i.id !== except && installationOf(i)?.adapter === 'claude' && installationOf(i)?.present)
    .reduce<Instance | undefined>((best, i) => (!best || score(i) < score(best) ? i : best), undefined)
}

/** Least-left window as a short "42%" for rows; '' without numbers. */
export function shortLeft(inst: Instance, s: Settings): { text: string; level: '' | 'warn' | 'crit' } {
  const w = tightest(mainOrg(inst), s)
  return w ? { text: `${Math.round(w.shown)}%`, level: levelOf(w, s) } : { text: '', level: '' }
}

export function isLaunching(inst: Instance): boolean {
  return !inst.running && !!ui.launching[inst.id]
}

/** The organisation shown first for an instance (respecting its org filter). */
export function mainOrg(inst: Instance): OrgUsage | undefined {
  return visibleOrgs(inst)[0]
}

export function windowsOf(org: OrgUsage | undefined, s: Settings) {
  const w5 = org?.quotas.find((q) => q.kind === 'fiveHour')
  const ww = org?.quotas.find((q) => q.kind === 'weekly')
  return { w5: w5 ? win(w5, s) : undefined, ww: ww ? win(ww, s) : undefined }
}

/** `''`, `'warn'` or `'crit'` from what is left (thresholds are on use) and pace. */
export function severity(org: OrgUsage | undefined, s: Settings): '' | 'warn' | 'crit' {
  const { w5, ww } = windowsOf(org, s)
  const ws = [w5, ww].filter(Boolean) as Win[]
  if (!ws.length) return ''
  const used = Math.max(...ws.map((w) => w.used))
  if (used >= s.usageCritPercent) return 'crit'
  if (used >= s.usageWarnPercent || ws.some((w) => w.paceWarn)) return 'warn'
  return ''
}

export function paceWarned(org: OrgUsage | undefined, s: Settings) {
  const { w5, ww } = windowsOf(org, s)
  return [w5, ww].some((w) => w?.paceWarn)
}

/** Rings for the floating launcher, outermost first. */
export function ringWindows(org: OrgUsage | undefined, s: Settings): Win[] {
  const { w5, ww } = windowsOf(org, s)
  switch (s.floatRing) {
    case 'none':
      return []
    case '5h':
      return w5 ? [w5] : ww ? [ww] : []
    case 'weekly':
      return ww ? [ww] : w5 ? [w5] : []
    case 'tight': {
      const ws = [w5, ww].filter(Boolean) as Win[]
      return ws.length ? [ws.reduce((a, b) => (a.left <= b.left ? a : b))] : []
    }
    default:
      return [ww, w5].filter(Boolean) as Win[]
  }
}

/** Ring geometry (100×100 viewBox): stroke, gap, and the outer box that keeps every ring
 *  clear of an icon of `size` px. */
export const RING = { stroke: 6, gap: 2.5, margin: 1, clearance: 2.5 }
export function ringBox(size: number, rings: number): number {
  if (!rings) return size
  const innerEdge = 50 - RING.margin - rings * RING.stroke - (rings - 1) * RING.gap
  return Math.ceil(size / ((innerEdge - RING.clearance) / 50))
}

/** The window with the least left (5-hour or weekly). */
export function tightest(org: OrgUsage | undefined, s: Settings): Win | undefined {
  const { w5, ww } = windowsOf(org, s)
  const ws = [w5, ww].filter(Boolean) as Win[]
  return ws.length ? ws.reduce((a, b) => (a.left <= b.left ? a : b)) : undefined
}

export function levelOf(w: Win, s: Settings): '' | 'warn' | 'crit' {
  return w.used >= s.usageCritPercent ? 'crit' : w.used >= s.usageWarnPercent || w.paceWarn ? 'warn' : ''
}
