// The main window's pages, in rail order. The window is the launcher's control panel: the
// floating bar is the product, and every page here — the instances first — is one part of
// setting it up. The rail, the settings page body, the command palette and "open settings at…"
// requests (tray, floating bar menu) all read this one list.
import { t } from './i18n.svelte'

export type PageId = 'instances' | 'float' | 'usage' | 'shortcuts' | 'taskbar' | 'appearance' | 'apps' | 'general' | 'data' | 'about'

export interface Page {
  id: PageId
  icon: string
  label: () => string
  /** `top`: the instances; `main`: settings people come back to; `end`: pinned to the rail's foot. */
  group: 'top' | 'main' | 'end'
}

export const PAGES: Page[] = [
  { id: 'instances', icon: 'grid', label: () => t('인스턴스'), group: 'top' },
  { id: 'float', icon: 'float', label: () => t('플로팅 바'), group: 'main' },
  { id: 'usage', icon: 'chart', label: () => t('사용량'), group: 'main' },
  { id: 'shortcuts', icon: 'keyboard', label: () => t('단축키'), group: 'main' },
  { id: 'taskbar', icon: 'taskbar', label: () => t('작업 표시줄'), group: 'main' },
  { id: 'appearance', icon: 'palette', label: () => t('모양'), group: 'main' },
  { id: 'apps', icon: 'layers', label: () => t('앱 관리'), group: 'main' },
  { id: 'general', icon: 'sliders', label: () => t('일반'), group: 'end' },
  { id: 'data', icon: 'database', label: () => t('데이터'), group: 'end' },
  { id: 'about', icon: 'info', label: () => t('정보'), group: 'end' },
]

/** Older names still sent by the launcher or saved in shortcuts. */
const ALIASES: Record<string, PageId> = { launcher: 'float', settings: 'general', home: 'instances' }

export function pageOf(id: string | null | undefined): PageId {
  const k = String(id ?? '')
  return (PAGES.find((p) => p.id === k)?.id ?? ALIASES[k] ?? 'general') as PageId
}
