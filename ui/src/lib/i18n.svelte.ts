// Korean source text is the key; English comes from the dictionary in ./en.ts.
// `t()` reads reactive state, so text re-renders when the language changes.
import { en } from './en'

export const locale = $state({ lang: 'ko' as 'ko' | 'en' })

export function resolveLang(setting: string): 'ko' | 'en' {
  if (setting === 'ko' || setting === 'en') return setting
  return (navigator.language || 'en').toLowerCase().startsWith('ko') ? 'ko' : 'en'
}

export function t(ko: string, vars?: Record<string, string | number>): string {
  let s = locale.lang === 'en' ? (en[ko] ?? ko) : ko
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.replaceAll(`{${k}}`, String(v))
  return s
}
