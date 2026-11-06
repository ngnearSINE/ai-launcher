<script lang="ts">
  import { listen } from '@tauri-apps/api/event'
  import { onMount, tick } from 'svelte'
  import { api } from '../lib/api'
  import { instanceMenu, launch, openSettings } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { appName, headroom, run, ui, visibleInstances } from '../lib/state.svelte'
  import Avatar from '../components/Avatar.svelte'
  import Icon from '../components/Icon.svelte'

  // Tray / hotkey panel: type to filter, arrows to move, Enter to launch, Esc to hide.
  let input = $state<HTMLInputElement | null>(null)
  let active = $state(0)
  const list = $derived(visibleInstances({ forQuick: true }))

  $effect(() => {
    void ui.search
    active = 0
  })

  onMount(() => {
    const un = listen('quick-opened', async () => {
      ui.search = ''
      active = 0
      await tick()
      input?.focus()
    })
    input?.focus()
    return () => {
      un.then((f) => f())
    }
  })

  async function go(i: number) {
    const inst = list[i]
    if (!inst) return
    if (inst.running) await run(api.focus(inst.id))
    else await launch(inst)
    api.hideWindow('quick')
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      active = Math.min(list.length - 1, active + 1)
      document.querySelector(`[data-q="${active}"]`)?.scrollIntoView({ block: 'nearest' })
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      active = Math.max(0, active - 1)
      document.querySelector(`[data-q="${active}"]`)?.scrollIntoView({ block: 'nearest' })
    } else if (e.key === 'Enter') {
      e.preventDefault()
      go(active)
    } else if (e.key === 'Escape') {
      e.preventDefault()
      if (ui.search) ui.search = ''
      else api.hideWindow('quick')
    }
  }
</script>

<div class="quick" role="dialog" aria-label={t('빠른 실행')}>
  <div class="search">
    <Icon name="search" size={16} />
    <input bind:this={input} bind:value={ui.search} placeholder={t('실행할 인스턴스…')} onkeydown={key} />
  </div>
  <div class="list" role="listbox">
    {#each list as inst, i (inst.id)}
      {@const h = headroom(inst)}
      <button
        class="item"
        class:on={i === active}
        data-q={i}
        role="option"
        aria-selected={i === active}
        onmouseenter={() => (active = i)}
        onclick={() => go(i)}
        oncontextmenu={(e) => instanceMenu(e, inst, true)}
      >
        <Avatar {inst} size={30} />
        <span class="txt">
          <span class="a truncate">{inst.alias}</span>
          <span class="b truncate">{[appName(inst) !== inst.alias ? appName(inst) : '', inst.usage?.report?.email ?? ''].filter(Boolean).join(' · ') || (inst.profile.kind === 'default' ? t('기본 프로필') : '')}</span>
        </span>
        {#if h !== null}
          <span class="mini" title="{Math.round(h)}%"><span class="f" class:warn={h >= 70} class:crit={h >= 90} style:width="{h}%"></span></span>
        {/if}
        {#if inst.running}<span class="run" title={t('실행 중')}></span>{/if}
        {#if i < 9}<span class="kbd">{i + 1}</span>{/if}
      </button>
    {:else}
      <div class="none muted">{t('찾는 인스턴스가 없어요')}</div>
    {/each}
  </div>
  <footer>
    <button class="btn ghost sm" onclick={() => api.showWindow('main')}><Icon name="app" size={13} /> {t('전체 보기')}</button>
    <button class="btn ghost sm" onclick={() => openSettings()}><Icon name="settings" size={13} /></button>
    <span class="hint faint"><span class="kbd">↵</span> {t('실행')} <span class="kbd">Esc</span> {t('닫기')}</span>
  </footer>
</div>

<svelte:window
  onkeydown={(e) => {
    if (document.activeElement !== input && /^[1-9]$/.test(e.key)) go(Number(e.key) - 1)
  }}
/>

<style>
  .quick {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border-radius: 12px;
    overflow: hidden;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px 10px;
    color: var(--text-3);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: 0;
    background: none;
    font-size: var(--fs-15);
    color: var(--text);
  }
  .list {
    flex: 1;
    overflow: auto;
    padding: 0 8px 8px;
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    text-align: left;
  }
  .item.on {
    background: var(--accent-soft);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .a {
    font-weight: 600;
    font-size: var(--fs-13);
  }
  .b {
    font-size: var(--fs-11);
    color: var(--text-2);
  }
  .mini {
    width: 42px;
    height: 4px;
    border-radius: 4px;
    background: var(--track);
    overflow: hidden;
  }
  .mini .f {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .mini .f.warn {
    background: var(--warn);
  }
  .mini .f.crit {
    background: var(--crit);
  }
  .run {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--run);
  }
  .none {
    padding: 24px;
    text-align: center;
  }
  footer {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 10px;
    border-top: 1px solid var(--line);
  }
  .hint {
    margin-left: auto;
    font-size: var(--fs-11);
    display: flex;
    align-items: center;
    gap: 4px;
  }
</style>
