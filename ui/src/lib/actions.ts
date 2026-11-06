// Instance actions shared by the cards, the quick panel, the floating launcher and the menus.
import { emitTo, listen } from '@tauri-apps/api/event'
import { api } from './api'
import { t } from './i18n.svelte'
import { confirmBox, dialogs, installationOf, openMenu, run, settings, toast, ui, type MenuItem } from './state.svelte'
import type { Instance } from './types'

export async function launch(inst: Instance) {
  await run(api.launch(inst.id))
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
  const removed = await run(api.deleteInstance(inst.id, r.checked))
  if (removed) {
    if (ui.drawer === inst.id) ui.drawer = null
    toast(removed.length ? t('삭제했어요 · 폴더 {n}개 정리', { n: removed.length }) : t('삭제했어요'), 'ok')
  }
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

/** Right-click menu for an instance. `compact` drops the editing entries (quick/float). */
export function instanceMenu(e: MouseEvent, inst: Instance, compact = false) {
  e.preventDefault()
  const install = installationOf(inst)
  const items: MenuItem[] = [
    inst.running
      ? { label: t('앞으로 가져오기'), icon: 'focus', action: () => run(api.focus(inst.id)) }
      : { label: t('실행'), icon: 'play', action: () => launch(inst), disabled: !install?.present },
    { label: t('창 닫기'), icon: 'x', disabled: !inst.running, action: () => run(api.closeWindows(inst.id)) },
    { label: t('다시 시작'), icon: 'refresh', disabled: !inst.running, action: () => run(api.restart(inst.id)) },
    { label: t('강제 종료'), icon: 'power', danger: true, disabled: !inst.running, action: () => quitInstance(inst) },
    { sep: true },
  ]
  if (!compact) {
    items.push(
      { label: t('편집…'), icon: 'sliders', hint: 'F2', action: () => edit(inst) },
      { label: inst.favorite ? t('즐겨찾기 해제') : t('즐겨찾기'), icon: 'star', action: () => run(api.updateInstance(inst.id, { favorite: !inst.favorite })) },
      { label: t('복제'), icon: 'copy', action: () => run(api.duplicateInstance(inst.id), t('복제했어요')) },
      { sep: true },
    )
  }
  if (install?.hasUsage) items.push({ label: t('사용량 새로고침'), icon: 'refresh', action: () => run(api.refreshUsage(inst.id)) })
  items.push({ label: t('프로필 폴더 열기'), icon: 'folder', action: () => run(api.openProfile(inst.id)) })
  if (!compact) {
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

/** Main window: react to requests from the quick panel / floating launcher. */
export function listenUiActions() {
  return listen<{ kind: string; payload: unknown }>('ui-action', (e) => {
    const { kind, payload } = e.payload
    if (kind === 'edit') ui.drawer = String(payload)
    if (kind === 'new') dialogs.newInstance = (payload as string | null) ?? true
    if (kind === 'settings') {
      ui.view = 'settings'
      ui.settingsTab = String(payload ?? 'general')
    }
  })
}
