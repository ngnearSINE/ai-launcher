// Instance actions shared by the sidebar, the detail pane, the command palette, the quick
// panel, the floating launcher and the menus. Every user-facing command ends up here.
import { emitTo, listen } from '@tauri-apps/api/event'
import { api } from './api'
import { t } from './i18n.svelte'
import { confirmBox, dialogs, dismissToast, installationOf, openMenu, run, settings, toast, ui, type MenuItem } from './state.svelte'
import type { Instance } from './types'

export async function launch(inst: Instance) {
  if (!inst.running) ui.launching[inst.id] = Date.now()
  const r = await run(api.launch(inst.id))
  if (r === undefined) delete ui.launching[inst.id]
}

/** The one primary action: bring it forward when running, start it otherwise. */
export function primary(inst: Instance) {
  if (!installationOf(inst)?.present) return
  if (inst.running) run(api.focus(inst.id))
  else launch(inst)
}

export async function quitInstance(inst: Instance) {
  if (settings().confirmQuitInstance) {
    const r = await confirmBox({
      title: t('{name} 종료', { name: inst.alias }),
      message: t('이 인스턴스의 모든 프로세스를 끝냅니다. 저장하지 않은 작업은 사라질 수 있어요.'),
      confirm: t('종료'),
      danger: true,
    })
    if (!r.ok) return
  }
  await run(api.quit(inst.id))
}

// Deletes wait a few seconds so they can be undone (toast button or Ctrl+Z); the instance is
// hidden meanwhile. Nothing is removed if the launcher quits inside that window.
const UNDO_MS = 6000
let pending: { id: string; timer: ReturnType<typeof setTimeout> } | null = null

function commitPending() {
  if (!pending) return
  const { id, timer } = pending
  clearTimeout(timer)
  pending = null
  void finishDelete(id)
}

async function finishDelete(id: string) {
  const data = deleteData.get(id) ?? false
  deleteData.delete(id)
  await run(api.deleteInstance(id, data))
  ui.pendingDelete = ui.pendingDelete.filter((x) => x !== id)
}
const deleteData = new Map<string, boolean>()

export async function deleteInstance(inst: Instance) {
  const managed = inst.profile.kind === 'dir'
  const r = await confirmBox({
    title: t('{name} 삭제', { name: inst.alias }),
    message: managed
      ? t('런처에서 이 인스턴스를 뺍니다. 프로필 폴더(로그인·기록)를 함께 지울지 고를 수 있어요.')
      : t('런처에서 이 인스턴스를 뺍니다. 앱의 기본 데이터 폴더는 건드리지 않아요.'),
    confirm: t('삭제'),
    danger: true,
    checkbox: managed
      ? { label: settings().deleteMovesToTrash ? t('프로필 폴더도 휴지통으로 보내기') : t('프로필 폴더도 영구 삭제'), value: settings().deleteProfileByDefault }
      : undefined,
  })
  if (!r.ok) return
  commitPending()
  deleteData.set(inst.id, r.checked)
  ui.pendingDelete = [...ui.pendingDelete, inst.id]
  if (ui.drawer === inst.id) ui.drawer = null
  if (ui.selected === inst.id) ui.selected = null
  pending = { id: inst.id, timer: setTimeout(commitPending, UNDO_MS) }
  toast(t('{name} 삭제했어요', { name: inst.alias }), 'info', { action: { label: t('되돌리기'), run: undoDelete }, duration: UNDO_MS })
}

/** Ctrl+Z / the toast's button. Returns whether there was something to undo. */
export function undoDelete(): boolean {
  if (!pending) return false
  const { id, timer } = pending
  clearTimeout(timer)
  pending = null
  deleteData.delete(id)
  ui.pendingDelete = ui.pendingDelete.filter((x) => x !== id)
  ui.selected = id
  dismissToast()
  return true
}

export function toggleFavorite(inst: Instance) {
  run(api.updateInstance(inst.id, { favorite: !inst.favorite }), inst.favorite ? t('고정을 풀었어요') : t('위에 고정했어요'))
}

/** Dialogs live in the main window; other windows ask it to open them. */
function inMain(kind: string, payload: unknown, here: () => void) {
  if (ui.label === 'main') return here()
  emitTo('main', 'ui-action', { kind, payload })
  api.showWindow('main')
  api.hideWindow(ui.label === 'quick' ? 'quick' : '')
}

export function edit(inst: Instance) {
  inMain('edit', inst.id, () => (ui.drawer = inst.id))
}

/** Shows the instance in the main pane (from the quick panel / float too). */
export function reveal(inst: Instance) {
  inMain('select', inst.id, () => {
    ui.view = 'home'
    ui.selected = inst.id
  })
}

export async function copyText(text: string, done: string) {
  try {
    await navigator.clipboard.writeText(text)
    toast(done, 'ok')
  } catch {
    toast(t('복사하지 못했어요'), 'error')
  }
}

/** Right-click / ⋯ menu for an instance. `compact` drops the editing entries (quick/float). */
export function instanceMenu(e: MouseEvent, inst: Instance, compact = false) {
  e.preventDefault()
  const install = installationOf(inst)
  const items: MenuItem[] = [
    inst.running
      ? { label: t('앞으로 가져오기'), icon: 'focus', hint: 'Enter', action: () => run(api.focus(inst.id)) }
      : { label: t('실행'), icon: 'play', hint: 'Enter', action: () => launch(inst), disabled: !install?.present },
    { label: t('창 닫기'), icon: 'x', disabled: !inst.running, action: () => run(api.closeWindows(inst.id)) },
    { label: t('다시 시작'), icon: 'refresh', disabled: !inst.running, action: () => run(api.restart(inst.id)) },
    { label: t('강제 종료'), icon: 'power', danger: true, disabled: !inst.running, action: () => quitInstance(inst) },
    { sep: true },
  ]
  if (!compact) {
    items.push(
      { label: t('편집…'), icon: 'sliders', hint: 'F2', action: () => edit(inst) },
      { label: inst.favorite ? t('고정 해제') : t('위에 고정'), icon: 'pin', hint: 'Ctrl+Shift+P', action: () => toggleFavorite(inst) },
      { label: t('복제'), icon: 'copy', action: () => run(api.duplicateInstance(inst.id), t('복제했어요')) },
      { sep: true },
    )
  } else {
    items.push({ label: t('자세히 보기'), icon: 'app', action: () => reveal(inst) })
  }
  if (install?.hasUsage) items.push({ label: t('사용량 새로고침'), icon: 'refresh', action: () => run(api.refreshUsage(inst.id)) })
  items.push({ label: t('프로필 폴더 열기'), icon: 'folder', action: () => run(api.openProfile(inst.id)) })
  if (!compact) {
    if (inst.profileDir) items.push({ label: t('프로필 경로 복사'), icon: 'copy', action: () => copyText(inst.profileDir!, t('경로를 복사했어요')) })
    items.push(
      { label: t('바탕화면에 바로가기'), icon: 'external', action: () => run(api.createShortcut(inst.id, 'desktop'), t('바탕화면에 만들었어요')) },
      {
        label: ui.snap?.os === 'linux' ? t('앱 메뉴에 추가') : t('시작 메뉴에 추가'),
        icon: 'pin',
        action: () => run(api.createShortcut(inst.id, 'startMenu'), t('추가했어요. 시작 메뉴에서 작업 표시줄에 고정할 수 있어요')),
      },
      { sep: true },
      { label: t('삭제…'), icon: 'trash', danger: true, hint: 'Del', action: () => deleteInstance(inst) },
    )
  } else {
    items.push({ sep: true }, { label: t('편집…'), icon: 'sliders', action: () => edit(inst) })
  }
  openMenu(e, items)
}

export function newInstance(installationId?: string) {
  inMain('new', installationId ?? null, () => (dialogs.newInstance = installationId ?? true))
}

export function openSettings(tab = 'general') {
  inMain('settings', tab, () => {
    ui.view = 'settings'
    ui.settingsTab = tab
  })
}

export function launchBestClaude() {
  run(api.launchBestClaude())
}

/** Main window: react to requests from the quick panel / floating launcher. */
export function listenUiActions() {
  return listen<{ kind: string; payload: unknown }>('ui-action', (e) => {
    const { kind, payload } = e.payload
    if (kind === 'edit') ui.drawer = String(payload)
    if (kind === 'new') dialogs.newInstance = (payload as string | null) ?? true
    if (kind === 'select') {
      ui.view = 'home'
      ui.selected = String(payload)
    }
    if (kind === 'settings') {
      ui.view = 'settings'
      ui.settingsTab = String(payload ?? 'general')
    }
    if (kind === 'help') ui.help = true
  })
}
