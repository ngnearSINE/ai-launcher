// The command list behind the palette (Ctrl+K) and the tray quick panel. Each source
// contributes commands; the palette only ranks and draws them. Shape follows Paseo's
// command-center contributions (id, group, rank, keywords, visibility, run) so more sources —
// or, later, plugins — can register the same way.
import { api } from './api'
import { deleteInstance, edit, launch, launchBestClaude, newInstance, openSettings, quitInstance, reveal, toggleFavorite } from './actions'
import { t } from './i18n.svelte'
import { keysOf } from './keymap.svelte'
import { PAGES } from './pages'
import { appName, dialogs, installationOf, run, settings, THEMES, ui, visibleInstances, type ThemeId } from './state.svelte'
import type { Instance } from './types'
import { bestClaude, mainOrg, windowsOf } from './usage'
import { sidebarOrder } from './sidebar.svelte'

export type Group = 'recommend' | 'instances' | 'actions' | 'instanceOps' | 'theme' | 'settings'

export interface Cmd {
  id: string
  group: Group
  title: string
  subtitle?: string
  icon?: string
  inst?: Instance
  /** Hidden words that also match (ranked below visible matches). */
  keywords?: string[]
  /** Only listed once something is typed — keeps the empty palette short. */
  query?: boolean
  shortcut?: string
  checked?: boolean
  danger?: boolean
  /** Newer first among equal matches. */
  recent?: number
  run: () => unknown
}

export const GROUP_LABEL: Record<Group, () => string> = {
  recommend: () => t('추천'),
  instances: () => t('인스턴스'),
  actions: () => t('동작'),
  instanceOps: () => t('인스턴스 동작'),
  theme: () => t('테마'),
  settings: () => t('설정'),
}
const GROUP_RANK: Group[] = ['recommend', 'instances', 'actions', 'instanceOps', 'theme', 'settings']

export const THEME_LABEL: Record<ThemeId, () => string> = {
  system: () => t('시스템'),
  light: () => t('밝게'),
  dark: () => t('어둡게'),
  claude: () => 'Claude',
  midnight: () => 'Midnight',
  black: () => t('완전 검정'),
}

function who(inst: Instance): string {
  const app = appName(inst)
  return [app !== inst.alias ? app : '', inst.usage?.report?.email ?? ''].filter(Boolean).join(' · ')
}

/** Every command for the given window. */
export function commands(where: 'main' | 'quick'): Cmd[] {
  const s = settings()
  // Same order as the sidebar (so Ctrl+1…9 means the same row everywhere), folded sections
  // last; the quick panel leaves out instances hidden from it.
  const shown = sidebarOrder()
  const list = [...shown, ...visibleInstances().filter((i) => !shown.includes(i))].filter((i) => where === 'main' || !i.advanced.hiddenInQuick)
  const out: Cmd[] = []

  const best = bestClaude(list, s)
  if (best && list.filter((i) => installationOf(i)?.adapter === 'claude').length > 1) {
    const { w5, ww } = windowsOf(mainOrg(best), s)
    const word = s.usageValue === 'used' ? t('사용') : t('남음')
    out.push({
      id: 'recommend:claude',
      group: 'recommend',
      title: t('지금 여유 있는 Claude: {name}', { name: best.alias }),
      subtitle: [w5 && `${t('5시간')} ${Math.round(w5.shown)}% ${word}`, ww && `${t('주간')} ${Math.round(ww.shown)}% ${word}`].filter(Boolean).join(' · '),
      inst: best,
      keywords: ['best', 'claude', '추천', '여유'],
      run: () => launchBestClaude(),
    })
  }

  for (const inst of list) {
    const present = !!installationOf(inst)?.present
    out.push({
      id: `inst:${inst.id}`,
      group: 'instances',
      title: inst.alias,
      subtitle: who(inst),
      inst,
      keywords: [appName(inst), inst.note].filter(Boolean),
      recent: inst.lastLaunchedAt ? Date.parse(inst.lastLaunchedAt) : 0,
      run: () => (inst.running ? run(api.focus(inst.id)) : present ? launch(inst) : reveal(inst)),
    })
    const ops: Omit<Cmd, 'group' | 'query' | 'inst'>[] = []
    if (inst.running) {
      ops.push({ id: `restart:${inst.id}`, title: t('{name} 다시 시작', { name: inst.alias }), icon: 'refresh', run: () => run(api.restart(inst.id)) })
      ops.push({ id: `quit:${inst.id}`, title: t('{name} 강제 종료', { name: inst.alias }), icon: 'power', danger: true, run: () => quitInstance(inst) })
    }
    ops.push({ id: `folder:${inst.id}`, title: t('{name} 프로필 폴더 열기', { name: inst.alias }), icon: 'folder', run: () => run(api.openProfile(inst.id)) })
    ops.push({ id: `edit:${inst.id}`, title: t('{name} 편집', { name: inst.alias }), icon: 'sliders', run: () => edit(inst) })
    if (where === 'main') {
      ops.push({ id: `pin:${inst.id}`, title: inst.favorite ? t('{name} 고정 해제', { name: inst.alias }) : t('{name} 위에 고정', { name: inst.alias }), icon: 'pin', run: () => toggleFavorite(inst) })
      ops.push({ id: `delete:${inst.id}`, title: t('{name} 삭제…', { name: inst.alias }), icon: 'trash', danger: true, run: () => deleteInstance(inst) })
    } else {
      ops.push({ id: `show:${inst.id}`, title: t('{name} 자세히 보기', { name: inst.alias }), icon: 'app', run: () => reveal(inst) })
    }
    for (const o of ops) out.push({ ...o, group: 'instanceOps', query: true, keywords: [inst.alias, appName(inst)] })
  }

  out.push(
    { id: 'new', group: 'actions', title: t('새 인스턴스'), icon: 'plus', shortcut: keysOf('newInstance'), run: () => newInstance() },
    { id: 'addApp', group: 'actions', title: t('앱 추가…'), icon: 'layers', query: true, keywords: ['app', 'install'], run: () => (where === 'main' ? (dialogs.addApp = true) : openSettings('apps')) },
    { id: 'floatSettings', group: 'actions', title: t('플로팅 바 설정'), icon: 'float', query: true, keywords: ['float', 'floating', 'magnet'], run: () => openSettings('float') },
    s.floatDocked
      ? { id: 'floatUndock', group: 'actions', title: t('플로팅 바를 작업 표시줄에서 떼기'), icon: 'float', query: true, keywords: ['float', 'taskbar', 'dock'], run: () => api.floatUndock() }
      : {
          id: 'floatOrientation',
          group: 'actions',
          title: s.floatOrientation === 'vertical' ? t('플로팅 바 가로로 놓기') : t('플로팅 바 세로로 놓기'),
          icon: 'float',
          query: true,
          keywords: ['float', 'horizontal', 'vertical'],
          run: () => run(api.updateSettings({ floatOrientation: s.floatOrientation === 'vertical' ? 'horizontal' : 'vertical' })),
        },
    { id: 'refresh', group: 'actions', title: t('사용량 모두 새로고침'), icon: 'refresh', keywords: ['usage', 'refresh'], run: () => run(api.refreshUsage(), t('새로 고치는 중이에요')) },
    { id: 'settings', group: 'actions', title: t('설정'), icon: 'settings', shortcut: keysOf('settings'), run: () => openSettings() },
    { id: 'help', group: 'actions', title: t('단축키 보기'), icon: 'keyboard', shortcut: keysOf('help'), query: where === 'quick', run: () => (where === 'main' ? (ui.help = true) : openHelp()) },
    { id: 'quitApp', group: 'actions', title: t('런처 종료'), icon: 'power', query: true, keywords: ['quit', 'exit'], run: () => api.quitApp() },
  )
  if (where === 'main') {
    out.push({ id: 'sidebar', group: 'actions', title: t('사이드바 접기·펴기'), icon: 'sidebar', query: true, shortcut: keysOf('sidebar'), run: () => toggleRail() })
  }
  for (const th of THEMES) {
    out.push({
      id: `theme:${th}`,
      group: 'theme',
      title: `${t('테마')} › ${THEME_LABEL[th]()}`,
      icon: 'palette',
      query: true,
      checked: s.theme === th,
      keywords: ['theme', 'dark', 'light'],
      run: () => run(api.updateSettings({ theme: th })),
    })
  }
  for (const p of PAGES.filter((p) => p.id !== 'instances')) {
    out.push({ id: `settings:${p.id}`, group: 'settings', title: `${t('설정')} › ${p.label()}`, icon: p.icon, query: true, run: () => openSettings(p.id) })
  }
  return out
}

function openHelp() {
  openSettings('shortcuts')
}

export function toggleRail() {
  ui.railPinned = !ui.railPinned
  localStorage.setItem('rail', ui.railPinned ? '1' : '0')
}

export function cycleTheme() {
  const i = THEMES.indexOf(settings().theme as ThemeId)
  const next = THEMES[(i + 1) % THEMES.length]
  run(api.updateSettings({ theme: next }), `${t('테마')} › ${THEME_LABEL[next]()}`)
}

// ---- ranking ---------------------------------------------------------------------------------

export interface Hit {
  cmd: Cmd
  score: number
  /** Matched [start, end) ranges in the title, for highlighting. */
  marks: [number, number][]
}
export interface Section {
  group: Group
  label: string
  hits: Hit[]
}

/** Word-order-free token match. A command whose tokens all hit visible text (title or
 *  subtitle) always ranks above one that needed hidden keywords; then title over subtitle,
 *  word starts over middles; then the most recently used. */
export function rank(query: string, cmds: Cmd[]): Section[] {
  const q = query.trim().toLowerCase()
  let hits: Hit[]
  if (!q) {
    hits = cmds.filter((c) => !c.query).map((c) => ({ cmd: c, score: 0, marks: [] }))
  } else {
    const tokens = q.split(/\s+/)
    hits = []
    for (const c of cmds) {
      const title = c.title.toLowerCase()
      const sub = (c.subtitle ?? '').toLowerCase()
      const hidden = (c.keywords ?? []).join(' ').toLowerCase()
      let score = 0
      let visible = true
      const marks: [number, number][] = []
      let ok = true
      for (const tk of tokens) {
        const ti = title.indexOf(tk)
        if (ti >= 0) {
          score += 30 + (ti === 0 || /\s/.test(title[ti - 1]) ? 15 : 0)
          marks.push([ti, ti + tk.length])
        } else if (sub.includes(tk)) score += 15
        else if (hidden.includes(tk)) {
          score += 5
          visible = false
        } else {
          ok = false
          break
        }
      }
      if (!ok) continue
      hits.push({ cmd: c, score: score + (visible ? 1000 : 0), marks })
    }
  }
  const bySection = new Map<Group, Hit[]>()
  for (const h of hits) {
    const arr = bySection.get(h.cmd.group) ?? []
    arr.push(h)
    bySection.set(h.cmd.group, arr)
  }
  const sections: Section[] = []
  for (const g of GROUP_RANK) {
    const arr = bySection.get(g)
    if (!arr?.length) continue
    if (q) arr.sort((a, b) => b.score - a.score || (b.cmd.recent ?? 0) - (a.cmd.recent ?? 0))
    sections.push({ group: g, label: GROUP_LABEL[g](), hits: arr })
  }
  // With a query the best section comes first, whatever its usual place.
  if (q) sections.sort((a, b) => b.hits[0].score - a.hits[0].score || GROUP_RANK.indexOf(a.group) - GROUP_RANK.indexOf(b.group))
  return sections
}
