// Usage model shared by cards, the drawer, the quick panel and the floating launcher.
// Same rules as the team's AI Usage widget: values show what is left by default, a pace tick
// marks where usage would be if spread evenly, and "pace warning" only fires when the window
// would run out well before it resets and half or less is left.
import type { Instance, OrgUsage, Quota, Settings } from './types'
import { visibleOrgs } from './state.svelte'

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
    runsOut,
    paceWarn,
  }
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

/** Grid rows a card spans: plain cards 1, usage cards 2, or 3 with several organisations/limits. */
export function cardSpan(inst: Instance, hasUsage: boolean): number {
  if (!hasUsage) return 1
  const org = mainOrg(inst)
  return (inst.usage?.report?.orgs.length ?? 0) > 1 || (org?.quotas.length ?? 0) > 3 ? 3 : 2
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
