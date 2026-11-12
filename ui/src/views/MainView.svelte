<script lang="ts">
  // Main window shell: title bar, the page rail, and the page. The window is the launcher's
  // settings; its first page, the instances, has the instance sidebar and one content pane
  // (overview or an instance). Every keyboard shortcut is resolved here through lib/keymap.
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { deleteInstance, edit, listenUiActions, newInstance, primary, quitInstance, toggleFavorite, undoDelete } from '../lib/actions'
  import { cycleTheme, toggleRail } from '../lib/commands.svelte'
  import { t } from '../lib/i18n.svelte'
  import { matches, typing } from '../lib/keymap.svelte'
  import { sidebarOrder } from '../lib/sidebar.svelte'
  import { dialogs, goPage, instanceById, run, ui } from '../lib/state.svelte'
  import AddAppDialog from '../components/AddAppDialog.svelte'
  import CommandPalette from '../components/CommandPalette.svelte'
  import Icon from '../components/Icon.svelte'
  import InstanceDrawer from '../components/InstanceDrawer.svelte'
  import NavRail from '../components/NavRail.svelte'
  import NewInstanceDialog from '../components/NewInstanceDialog.svelte'
  import Settings from '../components/Settings.svelte'
  import ShortcutHelp from '../components/ShortcutHelp.svelte'
  import Sidebar from '../components/Sidebar.svelte'
  import InstanceDetail from './InstanceDetail.svelte'
  import Overview from './Overview.svelte'

  const win = getCurrentWindow()
  const selected = $derived(instanceById(ui.selected))
  const rail = $derived(ui.railPinned || ui.narrow)
  const home = $derived(ui.page === 'instances')
  const update = $derived(ui.snap?.update ?? { status: 'idle' as const })

  function measure() {
    ui.narrow = innerWidth < 840
  }
  onMount(() => {
    measure()
    const un = listenUiActions()
    return () => {
      un.then((f) => f())
    }
  })

  // Hold Ctrl (or Alt) a moment to see which number launches which row.
  let holdTimer: ReturnType<typeof setTimeout> | undefined
  function stopHold() {
    clearTimeout(holdTimer)
    ui.numbers = false
  }

  function step(d: number) {
    const order = sidebarOrder()
    if (!order.length) return
    const i = order.findIndex((x) => x.id === ui.selected)
    const next = order[i < 0 ? (d > 0 ? 0 : order.length - 1) : (i + d + order.length) % order.length]
    goPage('instances')
    ui.selected = next.id
    queueMicrotask(() => document.querySelector(`[data-sort-id="${next.id}"]`)?.scrollIntoView({ block: 'nearest' }))
  }

  function key(e: KeyboardEvent) {
    if ((e.key === 'Control' || e.key === 'Alt') && !e.repeat) {
      clearTimeout(holdTimer)
      holdTimer = setTimeout(() => (ui.numbers = true), 150)
      return
    }
    if (e.key !== 'Control' && e.key !== 'Alt') clearTimeout(holdTimer)
    // Dialogs and the palette handle their own keys.
    if (document.querySelector('.scrim') || ui.palette) return
    const editing = typing(e)
    const act = (id: string) => matches(e, id)
    const sel = selected
    const run1 = (f: () => unknown) => {
      e.preventDefault()
      f()
    }

    if (act('palette')) return run1(() => (ui.palette = true))
    if (/^Digit[1-9]$/.test(e.code) && e.ctrlKey && !e.altKey && !e.shiftKey) {
      const inst = sidebarOrder()[Number(e.code.slice(5)) - 1]
      if (inst) run1(() => primary(inst))
      return
    }
    if (act('newInstance')) return run1(() => newInstance())
    if (act('settings')) return run1(() => goPage(home ? ui.lastSettings : 'instances'))
    if (act('sidebar')) return run1(toggleRail)
    if (act('theme')) return run1(cycleTheme)
    if (act('overview')) return run1(() => (goPage('instances'), (ui.selected = null)))
    if (act('next')) return run1(() => step(1))
    if (act('prev')) return run1(() => step(-1))
    if (act('pin') && sel) return run1(() => toggleFavorite(sel))
    if (editing) return
    if (act('help')) return run1(() => (ui.help = true))
    if (act('undo')) {
      if (undoDelete()) e.preventDefault()
      return
    }
    if (e.key === 'Escape') {
      if (!home) return run1(() => goPage('instances'))
      if (ui.selected && !ui.drawer) return run1(() => (ui.selected = null))
      return
    }
    if (!home) return
    if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && !e.altKey && !e.ctrlKey) return run1(() => step(e.key === 'ArrowDown' ? 1 : -1))
    if (!sel) return
    // Enter on a focused button belongs to the button.
    if (act('launch') && !(e.target as HTMLElement).closest('button')) return run1(() => primary(sel))
    if (act('edit')) return run1(() => edit(sel))
    if (act('delete')) return run1(() => deleteInstance(sel))
    if (act('restart') && sel.running) return run1(() => run(api.restart(sel.id)))
    if (act('quit') && sel.running) return run1(() => quitInstance(sel))
  }
</script>

<svelte:window onkeydown={key} onkeyup={stopHold} onblur={stopHold} onresize={measure} />

<div class="shell">
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <img src="/icon.png" alt="" /><span data-tauri-drag-region>{t('AI 런처')}</span>
      {#if home}
        <button class="icon-btn sm" title={t('사이드바 접기·펴기') + ' (Ctrl+B)'} aria-label={t('사이드바 접기·펴기')} onclick={toggleRail} disabled={ui.narrow}>
          <Icon name="sidebar" size={16} />
        </button>
      {/if}
    </div>
    <div class="mid" data-tauri-drag-region></div>
    <div class="right" data-tauri-drag-region>
      {#if update.status === 'ready'}
        <button class="upd ready" title={t('다시 시작해 {v}(으)로 업데이트', { v: update.version })} onclick={() => run(api.installUpdate())}>
          <Icon name="refresh" size={13} /><span>{t('업데이트 {v}', { v: update.version })}</span>
        </button>
      {:else if update.status === 'downloading'}
        <span class="upd" title={t('{v} 받는 중', { v: update.version })}><span class="ring" style:--p={update.percent}></span><span>{t('업데이트 받는 중')}</span></span>
      {:else if update.status === 'installing'}
        <span class="upd"><span class="ring spin"></span><span>{t('업데이트 설치 중')}</span></span>
      {/if}
      <div class="winctl">
        <button aria-label={t('최소화')} onclick={() => win.minimize()}><Icon name="minus" size={14} /></button>
        <button aria-label={t('최대화')} onclick={() => win.toggleMaximize()}><Icon name="square" size={12} /></button>
        <button class="close" aria-label={t('닫기')} onclick={() => win.close()}><Icon name="x" size={15} /></button>
      </div>
    </div>
  </header>

  <div class="body">
    <NavRail />
    {#if home}
      <div class="page">
        <Sidebar {rail} />
        <main class="pane">
          {#if selected}
            {#key selected.id}<InstanceDetail inst={selected} />{/key}
          {:else}
            <Overview />
          {/if}
        </main>
      </div>
    {:else}
      <main class="page pane settings-wrap"><Settings /></main>
    {/if}
  </div>
</div>

<InstanceDrawer />
{#if dialogs.newInstance}<NewInstanceDialog />{/if}
{#if dialogs.addApp}<AddAppDialog />{/if}
{#if ui.palette}<CommandPalette onclose={() => (ui.palette = false)} />{/if}
{#if ui.help}<ShortcutHelp />{/if}

<style>
  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .titlebar {
    height: 40px;
    flex: none;
    display: flex;
    align-items: center;
  }
  .brand {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 0;
    font-weight: 650;
    font-size: var(--fs-13);
    letter-spacing: -0.01em;
  }
  /* The app icon sits over the rail's centre line. */
  .brand img {
    width: 18px;
    height: 18px;
    margin-left: 29px;
    margin-right: 4px;
    pointer-events: none;
  }
  .brand .icon-btn {
    margin-left: 8px;
  }
  .mid {
    flex: 1;
    min-width: 0;
    display: flex;
    justify-content: center;
    padding: 0 12px;
  }
  /* Update pill: quiet while downloading, accent when it only needs a restart. */
  .upd {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 24px;
    padding: 0 10px 0 8px;
    margin-right: 4px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: transparent;
    color: var(--text-2);
    font-size: var(--fs-12);
    white-space: nowrap;
  }
  .upd.ready {
    border-color: transparent;
    background: var(--accent);
    color: var(--accent-contrast);
    font-weight: 600;
    cursor: pointer;
    transition: filter var(--dur) var(--ease);
  }
  .upd.ready:hover {
    filter: brightness(1.08);
  }
  .upd .ring {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: conic-gradient(var(--accent) calc(var(--p, 25) * 1%), var(--line) 0);
    mask: radial-gradient(circle, transparent 3.5px, #000 4px);
  }
  .upd .ring.spin {
    animation: spin 900ms linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    height: 100%;
  }
  .winctl {
    display: flex;
    height: 100%;
    margin-left: 4px;
  }
  .winctl button {
    width: 46px;
    height: 100%;
    border: 0;
    background: transparent;
    color: var(--text-2);
    display: grid;
    place-items: center;
  }
  .winctl button:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .winctl .close:hover {
    background: #e81123;
    color: white;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .page {
    flex: 1;
    min-width: 0;
    display: flex;
  }
  /* The content pane is one step up from the window backdrop, with a soft top-left corner. */
  .pane {
    flex: 1;
    min-width: 0;
    overflow: auto;
    background: var(--layer);
    border-top: 1px solid var(--line);
    border-left: 1px solid var(--line);
    border-top-left-radius: var(--r-lg);
  }
  .settings-wrap {
    overflow: hidden;
  }
</style>
