// Sidebar sections and row order. The sidebar draws them; the key handler (Ctrl+1…9, arrows,
// Ctrl+[ ]) walks the same order, so numbers and arrows always match what is on screen.
import { t } from './i18n.svelte'
import { installationOf, settings, ui, visibleInstances } from './state.svelte'
import type { Instance } from './types'
import { statusOf } from './usage'

export interface SidebarSection {
  id: string
  label: string
  items: Instance[]
}

const collapsed: Record<string, boolean> = (() => {
  try {
    return JSON.parse(localStorage.getItem('sidebar-collapsed') || '{}')
  } catch {
    return {}
  }
})()
export const fold = $state(collapsed)
export function toggleFold(id: string) {
  fold[id] = !fold[id]
  try {
    localStorage.setItem('sidebar-collapsed', JSON.stringify(fold))
  } catch {
    /* per-viewer convenience only */
  }
}

/** Pinned (favourites) first, then the chosen grouping. Empty sections are left out. */
export function sidebarSections(): SidebarSection[] {
  if (!ui.snap) return []
  const s = settings()
  const all = visibleInstances()
  const pinned = all.filter((i) => i.favorite)
  const rest = all.filter((i) => !i.favorite)
  const out: SidebarSection[] = []
  if (pinned.length) out.push({ id: 'pinned', label: t('고정'), items: pinned })

  switch (s.sidebarGroup) {
    case 'status': {
      const by = (st: string[]) => rest.filter((i) => st.includes(statusOf(i, s)))
      out.push(
        { id: 'needs', label: t('확인 필요'), items: by(['needs']) },
        { id: 'low', label: t('한도 임박'), items: by(['crit', 'warn']) },
        { id: 'running', label: t('실행 중'), items: by(['running']) },
        { id: 'idle', label: t('대기'), items: by(['idle']) },
      )
      break
    }
    case 'app': {
      for (const app of ui.snap.installations) {
        out.push({ id: `app:${app.id}`, label: app.name, items: rest.filter((i) => i.installationId === app.id) })
      }
      break
    }
    case 'group': {
      for (const g of [...ui.snap.groups].sort((a, b) => a.order - b.order)) {
        out.push({ id: `group:${g.id}`, label: g.name, items: rest.filter((i) => i.groupId === g.id) })
      }
      out.push({ id: 'group:none', label: t('그룹 없음'), items: rest.filter((i) => !i.groupId || !ui.snap!.groups.some((g) => g.id === i.groupId)) })
      break
    }
    default:
      out.push({ id: 'all', label: t('인스턴스'), items: rest })
  }
  return out.filter((sec) => sec.items.length)
}

/** Rows in screen order, skipping folded sections. */
export function sidebarOrder(): Instance[] {
  return sidebarSections().flatMap((sec) => (fold[sec.id] ? [] : sec.items))
}

/** Instances that report Claude usage, for the footer summary and the overview. */
export function usageInstances(): Instance[] {
  return visibleInstances().filter((i) => installationOf(i)?.hasUsage && i.effective.usageEnabled)
}
