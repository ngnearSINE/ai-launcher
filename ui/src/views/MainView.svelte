<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import { deleteInstance, edit, launch, listenUiActions, newInstance } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { dialogs, run, ui, visibleInstances } from '../lib/state.svelte'
  import Icon from '../components/Icon.svelte'
  import InstanceCard from '../components/InstanceCard.svelte'
  import InstanceDrawer from '../components/InstanceDrawer.svelte'
  import NewInstanceDialog from '../components/NewInstanceDialog.svelte'
  import AddAppDialog from '../components/AddAppDialog.svelte'
  import Settings from '../components/Settings.svelte'

  const win = getCurrentWindow()
  let searchEl = $state<HTMLInputElement | null>(null)
  let dragId = $state<string | null>(null)

  const list = $derived(visibleInstances())
  const snap = $derived(ui.snap!)
  const runningCount = $derived(snap.instances.filter((i) => i.running).length)
  const filters = $derived([
    { id: 'all', label: t('전체'), count: snap.instances.length },
    { id: 'running', label: t('실행 중'), count: runningCount },
    ...(snap.instances.some((i) => i.favorite) ? [{ id: 'favorite', label: t('즐겨찾기'), count: snap.instances.filter((i) => i.favorite).length }] : []),
    ...snap.installations
      .filter((a) => !a.hidden && snap.instances.some((i) => i.installationId === a.id))
      .map((a) => ({ id: `app:${a.id}`, label: a.name, count: snap.instances.filter((i) => i.installationId === a.id).length })),
    ...snap.groups.map((g) => ({ id: `group:${g.id}`, label: g.name, count: snap.instances.filter((i) => i.groupId === g.id).length })),
  ])

  onMount(() => {
    const un = listenUiActions()
    return () => {
      un.then((f) => f())
    }
  })

  function setLayout(l: 'grid' | 'list') {
    ui.layout = l
    localStorage.setItem('layout', l)
  }

  function onDrop(target: string) {
    if (!dragId || dragId === target) return
    const ids = snap.instances.map((i) => i.id)
    const from = ids.indexOf(dragId)
    const to = ids.indexOf(target)
    ids.splice(to, 0, ids.splice(from, 1)[0])
    dragId = null
    run(api.reorder(ids))
  }

  function typing(e: KeyboardEvent) {
    const el = e.target as HTMLElement
    return el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.tagName === 'SELECT' || el.isContentEditable
  }
  function key(e: KeyboardEvent) {
    if (document.querySelector('.scrim')) return
    const mod = e.ctrlKey || e.metaKey
    if (mod && e.key.toLowerCase() === 'n') {
      e.preventDefault()
      newInstance()
    } else if ((mod && e.key.toLowerCase() === 'f') || (e.key === '/' && !typing(e))) {
      e.preventDefault()
      ui.view = 'home'
      searchEl?.focus()
    } else if (mod && e.key === ',') {
      e.preventDefault()
      ui.view = ui.view === 'settings' ? 'home' : 'settings'
    } else if (!typing(e) && !mod && /^[1-9]$/.test(e.key) && ui.view === 'home') {
      const inst = list[Number(e.key) - 1]
      if (inst) launch(inst)
    } else if (!typing(e) && (e.key === 'Delete' || e.key === 'F2')) {
      const id = (document.activeElement as HTMLElement | null)?.closest('[data-index]')?.getAttribute('data-index')
      const inst = id ? list[Number(id)] : undefined
      if (inst) e.key === 'Delete' ? deleteInstance(inst) : edit(inst)
    } else if (e.key === 'Escape' && ui.search && document.activeElement === searchEl) {
      ui.search = ''
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="shell">
  <header class="titlebar" data-tauri-drag-region>
    <div class="brand" data-tauri-drag-region>
      <img src="/icon.png" alt="" />
      <span>{t('AI 런처')}</span>
    </div>
    <label class="search">
      <Icon name="search" size={14} />
      <input bind:this={searchEl} bind:value={ui.search} placeholder={t('인스턴스 검색')} onfocus={() => (ui.view = 'home')} />
      {#if !ui.search}<span class="kbd">/</span>{/if}
    </label>
    <div class="right" data-tauri-drag-region>
      <button class="btn primary sm" onclick={() => newInstance()} title="Ctrl+N"><Icon name="plus" size={14} /> {t('새 인스턴스')}</button>
      <button class="icon-btn" class:active={ui.view === 'settings'} title={t('설정') + ' (Ctrl+,)'} onclick={() => (ui.view = ui.view === 'settings' ? 'home' : 'settings')}><Icon name="settings" size={17} /></button>
      <div class="winctl">
        <button aria-label={t('최소화')} onclick={() => win.minimize()}><Icon name="minus" size={14} /></button>
        <button aria-label={t('최대화')} onclick={() => win.toggleMaximize()}><Icon name="square" size={12} /></button>
        <button class="close" aria-label={t('닫기')} onclick={() => win.close()}><Icon name="x" size={15} /></button>
      </div>
    </div>
  </header>

  {#if ui.view === 'settings'}
    <main class="settings-wrap"><Settings /></main>
  {:else}
    <div class="toolbar">
      <div class="chips">
        {#each filters as f (f.id)}
          <button class="chip" class:on={ui.filter === f.id} onclick={() => (ui.filter = f.id)}>
            {f.label} <span class="n num">{f.count}</span>
          </button>
        {/each}
      </div>
      <div class="views">
        <button class="icon-btn sm" class:active={ui.layout === 'grid'} aria-label={t('카드')} onclick={() => setLayout('grid')}><Icon name="grid" size={15} /></button>
        <button class="icon-btn sm" class:active={ui.layout === 'list'} aria-label={t('목록')} onclick={() => setLayout('list')}><Icon name="list" size={15} /></button>
      </div>
    </div>

    <main class="content">
      {#if !snap.instances.length}
        <div class="empty">
          <img src="/icon.png" alt="" />
          <h2>{t('아직 인스턴스가 없어요')}</h2>
          <p class="muted">{t('설치된 앱을 찾지 못했어요. 앱을 추가하거나 다시 찾아보세요.')}</p>
          <div class="row">
            <button class="btn primary" onclick={() => (dialogs.addApp = true)}>{t('앱 추가')}</button>
            <button class="btn" onclick={() => run(api.rescan())}>{t('다시 찾기')}</button>
          </div>
        </div>
      {:else if !list.length}
        <div class="empty small">
          <p class="muted">{t('조건에 맞는 인스턴스가 없어요')}</p>
        </div>
      {:else}
        <div class="cards" class:list={ui.layout === 'list'}>
          {#each list as inst, i (inst.id)}
            <InstanceCard
              {inst}
              index={i}
              list={ui.layout === 'list'}
              dragging={dragId === inst.id}
              ondragstart={(e) => {
                dragId = inst.id
                e.dataTransfer?.setData('text/plain', inst.id)
              }}
              ondrop={() => onDrop(inst.id)}
            />
          {/each}
          {#if ui.layout === 'grid'}
            <button class="add-card" onclick={() => newInstance()}>
              <Icon name="plus" size={22} />
              <span>{t('새 인스턴스')}</span>
            </button>
          {/if}
        </div>
      {/if}
    </main>
  {/if}
</div>

<InstanceDrawer />
{#if dialogs.newInstance}<NewInstanceDialog />{/if}
{#if dialogs.addApp}<AddAppDialog />{/if}

<style>
  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }
  .titlebar {
    height: 48px;
    flex: none;
    display: grid;
    grid-template-columns: 1fr minmax(200px, 360px) 1fr;
    align-items: center;
    gap: 12px;
    padding: 0 0 0 14px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 650;
    font-size: var(--fs-13);
    letter-spacing: -0.01em;
  }
  .brand img {
    width: 20px;
    height: 20px;
    pointer-events: none;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 8px 0 10px;
    border-radius: 9px;
    background: var(--well);
    border: 1px solid transparent;
    color: var(--text-3);
    transition: border-color var(--dur) var(--ease), background var(--dur) var(--ease);
  }
  .search:focus-within {
    border-color: color-mix(in srgb, var(--accent) 60%, transparent);
    background: var(--surface-solid);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: 0;
    background: none;
    min-width: 0;
    color: var(--text);
  }
  .right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
    height: 100%;
  }
  .icon-btn.active {
    color: var(--accent);
    background: var(--accent-softer);
  }
  .winctl {
    display: flex;
    height: 100%;
    margin-left: 6px;
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
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 4px 20px 10px;
  }
  .chips {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 11px;
    border-radius: var(--r-pill);
    border: 1px solid var(--line);
    background: transparent;
    color: var(--text-2);
    font-size: var(--fs-12);
    white-space: nowrap;
    transition: all var(--dur) var(--ease);
  }
  .chip:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .chip.on {
    background: var(--text);
    border-color: var(--text);
    color: var(--bg-solid);
  }
  .chip .n {
    opacity: 0.6;
    font-size: var(--fs-11);
  }
  .views {
    display: flex;
    gap: 2px;
  }
  .content {
    flex: 1;
    overflow: auto;
    padding: 4px 20px 24px;
  }
  .cards {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
    gap: 14px;
    align-items: start;
  }
  .cards.list {
    grid-template-columns: 1fr;
    gap: 8px;
  }
  .add-card {
    min-height: 112px;
    border-radius: var(--r-card);
    border: 1.5px dashed var(--line-strong);
    background: transparent;
    color: var(--text-3);
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 6px;
    font-size: var(--fs-13);
    transition: all var(--dur) var(--ease);
  }
  .add-card:hover {
    color: var(--accent);
    border-color: var(--accent);
    background: var(--accent-softer);
  }
  .empty {
    height: 100%;
    display: grid;
    place-content: center;
    justify-items: center;
    text-align: center;
    gap: 6px;
  }
  .empty img {
    width: 72px;
    height: 72px;
    margin-bottom: 8px;
  }
  .empty h2 {
    margin: 0;
    font-size: var(--fs-17);
  }
  .empty .row {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .settings-wrap {
    flex: 1;
    min-height: 0;
    border-top: 1px solid var(--line);
  }
</style>
