<script lang="ts">
  import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window'
  import { api } from '../lib/api'
  import { instanceMenu, launch, openSettings } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { pct, until } from '../lib/format'
  import { openMenu, run, settings, ui, visibleInstances } from '../lib/state.svelte'
  import { levelOf, mainOrg, ringBox, ringWindows, windowsOf } from '../lib/usage'
  import type { Instance } from '../lib/types'
  import Avatar from '../components/Avatar.svelte'
  import Icon from '../components/Icon.svelte'

  // Always-on-top strip of instance icons. Rings follow the "Usage ring" setting:
  // both = outer weekly + inner 5-hour, or one of 5-hour / weekly / whichever is tighter.
  const s = $derived(settings())
  const list = $derived(visibleInstances({ forQuick: true }))
  const vertical = $derived(s.floatOrientation === 'vertical')
  const size = $derived(s.floatSize)
  // Every cell is as big as the largest ring set, so icons line up whether or not they have usage.
  const maxRings = $derived(s.floatRing === 'none' ? 0 : s.floatRing === 'both' ? 2 : 1)
  const cell = $derived(ringBox(size - 6, maxRings) + 6)

  $effect(() => {
    const n = Math.max(1, list.length)
    const along = 30 + n * cell + 8
    const across = cell + 12
    getCurrentWindow().setSize(new LogicalSize(vertical ? across : along, vertical ? along : across))
  })

  function rings(inst: Instance) {
    return ringWindows(mainOrg(inst), s).map((w) => ({ value: w.shown, paceAt: w.paceAt, level: levelOf(w, s) }))
  }
  function tip(inst: Instance) {
    const { w5, ww } = windowsOf(mainOrg(inst), s)
    const word = s.usageValue === 'used' ? t('사용') : t('남음')
    const parts = [inst.alias]
    if (inst.usage?.report?.email) parts.push(inst.usage.report.email)
    if (w5) parts.push(`${t('5시간')} ${pct(w5.shown)} ${word}${w5.q.resetsAt ? ` · ${until(w5.q.resetsAt, ui.now)}` : ''}`)
    if (ww) parts.push(`${t('주간')} ${pct(ww.shown)} ${word}${ww.q.resetsAt ? ` · ${until(ww.q.resetsAt, ui.now)}` : ''}`)
    if (inst.running) parts.push(t('실행 중'))
    return parts.join('\n')
  }
  function click(inst: Instance) {
    if (inst.running) run(api.focus(inst.id))
    else launch(inst)
  }
  function stripMenu(e: MouseEvent) {
    e.preventDefault()
    openMenu(e, [
      { label: t('런처 창으로 돌아가기'), icon: 'app', action: () => api.switchView('main') },
      { label: t('플로팅 설정…'), icon: 'settings', action: () => openSettings('launcher') },
      { sep: true },
      { label: t('플로팅 런처 닫기'), icon: 'x', action: () => api.setFloat(false) },
    ])
  }
</script>

<div
  class="float"
  class:h={!vertical}
  style:opacity={s.floatOpacity / 100}
  role="toolbar"
  tabindex="-1"
  oncontextmenu={stripMenu}
  ondblclick={(e) => {
    if (e.target === e.currentTarget) api.switchView('main')
  }}
>
  <div class="bar">
    <span class="grip" data-tauri-drag-region title={t('끌어서 옮기기 · 빈 곳 더블클릭: 런처 창')}><Icon name="grip" size={12} /></span>
    <button class="expand" title={t('런처 창으로 돌아가기') + (s.floatHotkeyEnabled ? ` (${s.floatHotkey})` : '')} onclick={() => api.switchView('main')}>
      <Icon name="external" size={12} />
    </button>
  </div>
  {#each list as inst (inst.id)}
    <button
      class="cell"
      class:on={inst.running}
      style:--cell="{cell}px"
      onclick={() => click(inst)}
      oncontextmenu={(e) => {
        e.stopPropagation()
        instanceMenu(e, inst, true)
      }}
      title={tip(inst)}
    >
      <Avatar {inst} size={size - 6} rings={rings(inst)} />
      <i class="run"></i>
    </button>
  {/each}
</div>

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
    gap: 2px;
    padding: 4px 4px 6px;
    border-radius: 20px;
    background: color-mix(in srgb, var(--surface-solid) 86%, transparent);
    backdrop-filter: blur(24px) saturate(1.6);
    border: 1px solid var(--line);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
    outline: none;
  }
  .float.h {
    flex-direction: row;
    padding: 4px 6px 4px 4px;
  }
  .bar {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    width: 100%;
  }
  .float.h .bar {
    width: auto;
    height: 100%;
    justify-content: center;
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
    width: 14px;
    height: auto;
    flex: 1;
  }
  .grip :global(svg) {
    pointer-events: none;
    transform: rotate(90deg);
  }
  .float.h .grip :global(svg) {
    transform: none;
  }
  .expand {
    width: 22px;
    height: 18px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--text-3);
  }
  .expand:hover {
    background: var(--surface-hover);
    color: var(--accent);
  }
  .cell {
    position: relative;
    width: var(--cell);
    height: var(--cell);
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 14px;
    background: transparent;
    transition: background var(--dur) var(--ease), transform var(--dur) var(--ease);
  }
  .cell:hover {
    background: var(--surface-hover);
    transform: scale(1.04);
  }
  /* Dock-style running indicator below (or beside) the icon. */
  .run {
    position: absolute;
    left: 50%;
    bottom: 1px;
    width: 4px;
    height: 4px;
    margin-left: -2px;
    border-radius: 50%;
    background: transparent;
  }
  .cell.on .run {
    background: var(--run);
  }
</style>
