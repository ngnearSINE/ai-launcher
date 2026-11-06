import { t } from './i18n.svelte'

/** "2시간 13분" / "3일 4시간" / "12분" until `iso`. */
export function until(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return ''
  let s = Math.max(0, Math.round((Date.parse(iso) - now) / 1000))
  const d = Math.floor(s / 86400)
  s -= d * 86400
  const h = Math.floor(s / 3600)
  s -= h * 3600
  const m = Math.floor(s / 60)
  if (d > 0) return t('{d}일 {h}시간', { d, h })
  if (h > 0) return t('{h}시간 {m}분', { h, m })
  return t('{m}분', { m: Math.max(1, m) })
}

/** "방금", "5분 전", "어제" … */
export function ago(iso: string | null | undefined, now = Date.now()): string {
  if (!iso) return t('없음')
  const s = Math.round((now - Date.parse(iso)) / 1000)
  if (s < 60) return t('방금')
  if (s < 3600) return t('{n}분 전', { n: Math.floor(s / 60) })
  if (s < 86400) return t('{n}시간 전', { n: Math.floor(s / 3600) })
  return t('{n}일 전', { n: Math.floor(s / 86400) })
}

export function bytes(n: number): string {
  if (n < 1024) return `${n} B`
  const u = ['KB', 'MB', 'GB', 'TB']
  let v = n / 1024
  let i = 0
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(v < 10 ? 1 : 0)} ${u[i]}`
}

/** "$12.50" / "₩12,000" — currency-aware amount for credits. */
export function money(n: number | null | undefined, currency: string | null | undefined): string {
  if (n === null || n === undefined) return ''
  try {
    return new Intl.NumberFormat(undefined, { style: 'currency', currency: currency || 'USD', maximumFractionDigits: 2 }).format(n)
  } catch {
    return n.toFixed(2)
  }
}

export function pct(n: number): string {
  return `${Math.round(n)}%`
}

/** Last path segment, for showing folder names compactly. */
export function baseName(p: string | null | undefined): string {
  if (!p) return ''
  const parts = p.split(/[\\/]/).filter(Boolean)
  return parts[parts.length - 1] ?? p
}
