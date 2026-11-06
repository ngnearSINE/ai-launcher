<script lang="ts">
  import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
  import { api } from '../lib/api'
  import { instanceMenu, launch, openSettings } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { headroom, openMenu, run, settings, visibleInstances } from '../lib/state.svelte'
  import Avatar from '../components/Avatar.svelte'
  import Icon from '../components/Icon.svelte'

  // Always-on-top strip of instance icons. The window resizes itself to its content.
  const s = $derived(settings())
  const list = $derived(visibleInstances({ forQuick: true }))
  const vertical = $derived(s.floatOrientation === 'vertical')
  const size = $derived(s.floatSize)
  let hover = $state<string | null>(null)

  $effect(() => {
    const n = Math.max(1, list.length)
    const cell = size + 10
    const along = 18 + n * cell + 6
    const across = size + 18
    getCurrentWindow().setSize(new LogicalSize(vertical ? across : along, vertical ? along : across))
  })

  function click(id: string) {
    const inst = list.find((i) => i.id === id)
    if (!inst) return
    if (inst.running) run(api.focus(inst.id))
    else launch(inst)
  }
  function stripMenu(e: MouseEvent) {
    e.preventDefault()
    openMenu(e, [
      { label: t('AI 런처 열기'), icon: 'app', action: () => api.showWindow('main') },
      { label: t('플로팅 설정…'), icon: 'settings', action: () => openSettings('launcher') },
      { sep: true },
      { label: t('플로팅 런처 숨기기'), icon: 'eye-off', action: () => api.setFloat(false) },
    ])
  }
</script>

<div class="float" class:h={!vertical} style:opacity={s.floatOpacity / 100} role="toolbar" tabindex="-1" oncontextmenu={stripMenu}>
  <div class="grip" data-tauri-drag-region title={t('끌어서 옮기기')}><Icon name="grip" size={12} /></div>
  {#each list as inst (inst.id)}
    {@const h = s.floatShowUsage ? headroom(inst) : null}
    <button
      class="cell"
      style:--s="{size}px"
      onclick={() => click(inst.id)}
      oncontextmenu={(e) => {
        e.stopPropagation()
        instanceMenu(e, inst, true)
      }}
      onmouseenter={() => (hover = inst.id)}
      onmouseleave={() => (hover = null)}
      title="{inst.alias}{h !== null ? ` · ${Math.round(h)}%` : ''}"
    >
      <Avatar {inst} size={size - 8} ring={h} />
    </button>
  {/each}
</div>
{#if hover}<span class="sr">{hover}</span>{/if}

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .float {
    position: absolute;
    inset: 4px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 4px 4px 8px;
    border-radius: 18px;
    background: color-mix(in srgb, var(--surface-solid) 82%, transparent);
    backdrop-filter: blur(24px) saturate(1.6);
    border: 1px solid var(--line);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
    outline: none;
  }
  .float.h {
    flex-direction: row;
    padding: 4px 8px 4px 4px;
  }
  .grip {
    width: 100%;
    height: 12px;
    display: grid;
    place-items: center;
    color: var(--text-3);
    cursor: grab;
  }
  .float.h .grip {
    width: 12px;
    height: 100%;
  }
  .grip :global(svg) {
    pointer-events: none;
    transform: rotate(90deg);
  }
  .float.h .grip :global(svg) {
    transform: none;
  }
  .cell {
    width: var(--s);
    height: var(--s);
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 12px;
    background: transparent;
    transition: background var(--dur) var(--ease), transform var(--dur) var(--ease);
  }
  .cell:hover {
    background: var(--surface-hover);
    transform: scale(1.06);
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
  }
</style>
