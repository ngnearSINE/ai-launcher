<script lang="ts">
  import { flip } from 'svelte/animate'
  import { api } from '../lib/api'
  import { instanceMenuItems, launch, openSettings, setHiddenInFloat } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { pct, until } from '../lib/format'
  import { popupNative } from '../lib/nativeMenu'
  import { run, settings, ui, visibleInstances } from '../lib/state.svelte'
  import { createSortable, mergeOrder } from '../lib/sortable.svelte'
  import { levelOf, mainOrg, ringBox, ringWindows, windowsOf } from '../lib/usage'
  import type { Instance } from '../lib/types'
  import Avatar from '../components/Avatar.svelte'
  import Icon from '../components/Icon.svelte'

  // The floating strip: a handle to move it, the instance icons, and a button that opens the
  // main window. It is laid out at 100 % here; the launcher zooms the whole page to the chosen
  // scale, so a corner drag scales every part alike. Dragging an end shows one more or one
  // fewer cell; icons beyond the visible cells scroll with the wheel.
  const s = $derived(settings())
  const all = $derived(visibleInstances())
  const list = $derived(visibleInstances({ forFloat: true }))
  // Docked in the taskbar it is the horizontal strip with two differences: the taskbar sets its
  // thickness (no corner or side grips) and it is flush (no margin). Everything else — handle,
  // cells, ends, reordering — is the same code.
  const docked = $derived(s.floatDocked)
  const vertical = $derived(!docked && s.floatOrientation === 'vertical')
  const thicknessFixed = $derived(docked)

  // Base geometry (logical px at 100 %). `fixed` + n × `cell` is the strip's length.
  const ICON = 38
  // Transparent margin (also where the window edge is grabbed) and padding, floating / docked.
  const FLOAT_EDGE = 4 + 5
  const DOCK_EDGE = 0 + 3
  const INSET = $derived(docked ? 0 : 4)
  const PAD = $derived(docked ? 3 : 5)
  const BORDER = 1
  const HANDLE = 22
  const FOOT = 32
  const maxRings = $derived(s.floatRing === 'none' ? 0 : s.floatRing === 'both' ? 2 : 1)
  const cell = $derived(ringBox(ICON, maxRings) + 6)
  const shape = (edge: number) => ({ across: cell + 2 * (edge + BORDER), fixed: 2 * (edge + BORDER) + HANDLE + FOOT })
  const across = $derived(cell + 2 * (INSET + PAD + BORDER))
  const fixed = $derived(2 * (INSET + PAD + BORDER) + HANDLE + FOOT)

  // Both shapes go along, so the launcher can preview the strip docked while it floats and
  // floating while it is docked (the ghosts shown on the way in and out of the taskbar).
  $effect(() => {
    const f = shape(FLOAT_EDGE)
    const d = shape(DOCK_EDGE)
    api.fitFloat({
      vertical, fixed, cell, across, total: Math.max(1, list.length),
      floatVertical: s.floatOrientation === 'vertical', floatFixed: f.fixed, floatAcross: f.across,
      dockFixed: d.fixed, dockAcross: d.across,
    })
  })

  // Drag icons to reorder; the order is the same one the main window uses.
  let viewport: HTMLDivElement | undefined = $state()
  const sorter = createSortable({
    ids: () => list.map((i) => i.id),
    commit: (ids) => run(api.reorder(mergeOrder((ui.snap?.instances ?? []).map((i) => i.id), ids))),
    enabled: () => s.sort === 'manual',
    scroller: () => viewport ?? null,
  })
  const shown = $derived(sorter.arrange(list, (i) => i.id))

  // ---- scrolling ------------------------------------------------------------------------------
  const sc = $state({ pos: 0, max: 0, len: 0, active: false })
  const before = $derived(Math.round(sc.pos / cell))
  const after = $derived(Math.max(0, Math.round((sc.max - sc.pos) / cell)))
  const pageCells = $derived(Math.max(1, Math.round(sc.len / cell)))
  const thumb = $derived(sc.max > 0 ? Math.max(0.18, sc.len / (sc.len + sc.max)) : 1)
  const thumbAt = $derived(sc.max > 0 ? (sc.pos / sc.max) * (1 - thumb) : 0)
  let target = -1
  let acc = 0
  let fadeTimer = 0

  function measure() {
    const v = viewport
    if (!v) return
    sc.pos = vertical ? v.scrollTop : v.scrollLeft
    sc.max = Math.max(0, vertical ? v.scrollHeight - v.clientHeight : v.scrollWidth - v.clientWidth)
    sc.len = vertical ? v.clientHeight : v.clientWidth
  }
  $effect(() => {
    const v = viewport
    if (!v) return
    const ro = new ResizeObserver(measure)
    ro.observe(v)
    if (v.firstElementChild) ro.observe(v.firstElementChild)
    measure()
    const done = () => (target = -1)
    v.addEventListener('scrollend', done)
    return () => {
      ro.disconnect()
      v.removeEventListener('scrollend', done)
    }
  })

  function reduced() {
    return s.reduceMotion || matchMedia('(prefers-reduced-motion: reduce)').matches
  }
  function wake() {
    sc.active = true
    clearTimeout(fadeTimer)
    fadeTimer = window.setTimeout(() => (sc.active = false), 900)
  }
  /** Scrolls by whole cells, so a cell never sits half outside. */
  function step(by: number) {
    const v = viewport
    if (!v || sc.max <= 0) return
    const from = target >= 0 ? target : Math.round(sc.pos / cell)
    const last = Math.round(sc.max / cell)
    target = Math.min(last, Math.max(0, from + by))
    v.scrollTo({ [vertical ? 'top' : 'left']: Math.min(sc.max, target * cell), behavior: reduced() ? 'auto' : 'smooth' })
    wake()
  }
  function wheel(e: WheelEvent) {
    if (sc.max <= 0) return
    e.preventDefault()
    acc += Math.abs(e.deltaY) >= Math.abs(e.deltaX) ? e.deltaY : e.deltaX
    if (Math.abs(acc) >= 40) {
      step(Math.sign(acc))
      acc = 0
    }
  }

  // ---- items ----------------------------------------------------------------------------------
  function rings(inst: Instance) {
    return ringWindows(mainOrg(inst), s).map((w) => ({ value: w.shown, paceAt: w.paceAt, level: levelOf(w, s), empty: w.left <= 0.5 }))
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
  const openMain = () => api.showWindow('main')
  const hotkey = $derived(s.floatHotkeyEnabled ? ` (${s.floatHotkey})` : '')

  function itemMenu(e: MouseEvent, inst: Instance) {
    e.preventDefault()
    e.stopPropagation()
    popupNative([...instanceMenuItems(inst, true), { sep: true }, { label: t('플로팅에서 숨기기'), action: () => setHiddenInFloat(inst, true) }])
  }
  function stripMenu(e: MouseEvent) {
    e.preventDefault()
    const hidden = all.filter((i) => i.advanced.hiddenInFloat)
    popupNative([
      { label: t('상세 창 열기'), action: openMain },
      docked
        ? { label: t('작업 표시줄에서 떼기'), action: () => api.floatUndock() }
        : { label: vertical ? t('가로로 놓기') : t('세로로 놓기'), action: () => run(api.updateSettings({ floatOrientation: vertical ? 'horizontal' : 'vertical' })) },
      ...(hidden.length
        ? [{ label: t('숨긴 아이콘 다시 표시 ({n})', { n: hidden.length }), action: () => hidden.forEach((i) => setHiddenInFloat(i, false)) }]
        : []),
      { label: t('플로팅 설정…'), action: () => openSettings('launcher') },
      { sep: true },
      { label: t('AI Launcher 종료'), action: () => api.quitApp() },
    ])
  }

  // ---- resizing (the window does it; see the launcher's magnet) -------------------------------
  type Dir = 'North' | 'South' | 'East' | 'West' | 'NorthEast' | 'NorthWest' | 'SouthEast' | 'SouthWest'
  function resize(e: PointerEvent, dir: Dir) {
    if (e.button !== 0) return
    e.preventDefault()
    void api.floatResize(dir)
  }
  const ends = $derived<[Dir, string][]>(vertical ? [['North', 'n'], ['South', 's']] : [['West', 'w'], ['East', 'e']])
  const sides = $derived<[Dir, string][]>(vertical ? [['West', 'w'], ['East', 'e']] : [['North', 'n'], ['South', 's']])
</script>

<div
  class="float"
  class:h={!vertical}
  class:docked
  style:opacity={s.floatOpacity / 100}
  style:--cell="{cell}px"
  style:--inset="{INSET}px"
  style:--pad="{PAD}px"
  style:--handle="{HANDLE}px"
  style:--foot="{FOOT}px"
  role="toolbar"
  tabindex="-1"
  oncontextmenu={stripMenu}
>
  <!-- The launcher moves the strip (floating: with the magnet, onto the taskbar to dock; docked:
       along the taskbar, or out of it); the page only says "pressed". -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="handle"
    title={docked ? t('좌우로 끌어 옮기기 · 작업 표시줄 밖으로 끌어내면 떨어져요') : t('끌어서 옮기기 · 작업 표시줄에 놓으면 붙어요')}
    onpointerdown={(e) => e.button === 0 && (e.preventDefault(), api.floatGrab())}
  >
    <span class="mesh"></span>
  </div>

  <div class="view" class:fade-a={before > 0} class:fade-b={after > 0} class:active={sc.active}>
    <div class="scroll" role="group" aria-label={t('인스턴스')} bind:this={viewport} onscroll={measure} onwheel={wheel} onpointerenter={() => sc.max > 0 && wake()}>
      <div class="track">
        {#each shown as inst (inst.id)}
          <button
            class="cell"
            class:on={inst.running}
            class:sort-placeholder={sorter.st.dragging === inst.id}
            data-sort-id={inst.id}
            animate:flip={{ duration: s.reduceMotion ? 0 : 200 }}
            onpointerdown={(e) => sorter.down(e, inst.id)}
            onclick={() => click(inst)}
            oncontextmenu={(e) => itemMenu(e, inst)}
            title={tip(inst)}
          >
            <Avatar {inst} size={ICON} rings={rings(inst)} />
            <i class="run"></i>
          </button>
        {/each}
      </div>
    </div>
    <button class="more a" class:show={before > 0} tabindex="-1" aria-label={t('위로 {n}개 더', { n: before })} onclick={() => step(-pageCells)}>
      <Icon name="chevron-down" size={11} stroke={2.2} /><span>{before}</span>
    </button>
    <button class="more b" class:show={after > 0} tabindex="-1" aria-label={t('아래로 {n}개 더', { n: after })} onclick={() => step(pageCells)}>
      <Icon name="chevron-down" size={11} stroke={2.2} /><span>{after}</span>
    </button>
    {#if sc.max > 0}
      <span class="rail" aria-hidden="true"><i style:--t={thumb} style:--at={thumbAt}></i></span>
    {/if}
  </div>

  <div class="foot">
    <button class="open" title={t('상세 창 열기') + hotkey} aria-label={t('상세 창 열기')} onclick={openMain}>
      <Icon name="window" size={16} />
    </button>
  </div>

  <!-- Corners: across = icon size, along = cells. Ends: cells only. Long sides: icon size only
       (the same cells, so the strip gets longer with them). Pointer-only affordances: the same
       is in Settings → Launcher as sliders. -->
  {#each thicknessFixed ? [] : [['NorthWest', 'nw'], ['NorthEast', 'ne'], ['SouthWest', 'sw'], ['SouthEast', 'se']] as [dir, cls] (cls)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="rz corner {cls}" title={vertical ? t('좌우로 끌면 아이콘 크기, 위아래로 끌면 칸 수') : t('위아래로 끌면 아이콘 크기, 좌우로 끌면 칸 수')} onpointerdown={(e) => resize(e, dir as Dir)}><i></i></span>
  {/each}
  {#each ends as [dir, cls] (cls)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="rz edge end {cls}" title={t('끌어서 칸 수 조절')} onpointerdown={(e) => resize(e, dir)}><i></i></span>
  {/each}
  {#each thicknessFixed ? [] : sides as [dir, cls] (cls)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <span class="rz edge side {cls}" title={t('끌어서 아이콘 크기 조절')} onpointerdown={(e) => resize(e, dir)}><i></i></span>
  {/each}
</div>

<style>
  @property --fa {
    syntax: '<length>';
    inherits: false;
    initial-value: 0px;
  }
  @property --fb {
    syntax: '<length>';
    inherits: false;
    initial-value: 0px;
  }
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
  }
  .float {
    position: absolute;
    inset: var(--inset);
    display: flex;
    flex-direction: column;
    padding: var(--pad);
    border-radius: 20px;
    background: color-mix(in srgb, var(--surface-solid) 88%, transparent);
    backdrop-filter: blur(24px) saturate(1.6);
    border: 1px solid var(--line);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
    outline: none;
  }
  .float.h {
    flex-direction: row;
  }

  /* ---- handle: a clear grab zone with a two-bar grabber ---- */
  .handle {
    flex: 0 0 var(--handle);
    display: grid;
    place-items: center;
    margin: 0 2px;
    border-radius: 10px;
    cursor: grab;
    transition: background var(--dur) var(--ease);
  }
  .float.h .handle {
    margin: 2px 0;
  }
  .handle:hover {
    background: var(--surface-hover);
  }
  .handle:active {
    cursor: grabbing;
  }
  /* A faint dot mesh: reads as "grip here" without drawing a control. */
  .mesh {
    width: 26px;
    height: 11px;
    pointer-events: none;
    background-image: radial-gradient(circle, var(--text-3) 0.95px, transparent 1.25px);
    background-size: 4px 4px;
    background-position: center;
    opacity: 0.7;
    transition: opacity var(--dur) var(--ease);
  }
  .float.h .mesh {
    width: 11px;
    height: 26px;
  }
  .handle:hover .mesh {
    opacity: 1;
    background-image: radial-gradient(circle, var(--accent) 0.95px, transparent 1.25px);
  }
  .float.docked .handle {
    cursor: grab;
  }
  /* In the taskbar: flat, no float shadow, softer corners, and opaque enough to cover whatever
     the taskbar draws underneath (the weather widget sits there when icons are left-aligned). */
  .float.docked {
    border-radius: 8px;
    box-shadow: none;
    background: color-mix(in srgb, var(--surface-solid) 96%, transparent);
    backdrop-filter: none;
  }

  /* ---- cells and scrolling ---- */
  .view {
    position: relative;
    flex: 1 1 auto;
    min-height: 0;
    min-width: 0;
    --fa: 0px;
    --fb: 0px;
    transition: --fa 220ms var(--ease), --fb 220ms var(--ease);
  }
  .view.fade-a {
    --fa: calc(var(--cell) * 0.55);
  }
  .view.fade-b {
    --fb: calc(var(--cell) * 0.55);
  }
  .scroll {
    position: absolute;
    inset: 0;
    overflow: hidden;
    overscroll-behavior: contain;
    -webkit-mask-image: linear-gradient(to bottom, transparent 0, #000 var(--fa), #000 calc(100% - var(--fb)), transparent 100%);
    mask-image: linear-gradient(to bottom, transparent 0, #000 var(--fa), #000 calc(100% - var(--fb)), transparent 100%);
  }
  .float.h .scroll {
    -webkit-mask-image: linear-gradient(to right, transparent 0, #000 var(--fa), #000 calc(100% - var(--fb)), transparent 100%);
    mask-image: linear-gradient(to right, transparent 0, #000 var(--fa), #000 calc(100% - var(--fb)), transparent 100%);
  }
  .track {
    display: flex;
    flex-direction: column;
  }
  .float.h .track {
    flex-direction: row;
  }
  .cell {
    position: relative;
    flex: 0 0 var(--cell);
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
  /* Dock-style running indicator below the icon. */
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

  /* "N more this way": a small capsule riding on the faded end, nudging toward what is hidden. */
  .more {
    position: absolute;
    left: 50%;
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 18px;
    padding: 0 7px 0 5px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface-solid) 92%, transparent);
    color: var(--text-2);
    font-size: var(--fs-12);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    line-height: 1;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.16);
    opacity: 0;
    pointer-events: none;
    transition: opacity 200ms var(--ease), transform 260ms var(--ease), color var(--dur) var(--ease);
  }
  .more.a {
    top: 2px;
    transform: translate(-50%, -6px) scale(0.9);
  }
  .more.b {
    bottom: 2px;
    transform: translate(-50%, 6px) scale(0.9);
  }
  .more.a :global(svg) {
    transform: rotate(180deg);
  }
  .more.show {
    opacity: 1;
    pointer-events: auto;
    transform: translate(-50%, 0) scale(1);
  }
  .more:hover {
    color: var(--accent);
    border-color: var(--accent-soft);
  }
  .more.b.show :global(svg) {
    animation: nudge-b 2.6s var(--ease) 0.6s infinite;
  }
  .more.a.show :global(svg) {
    animation: nudge-a 2.6s var(--ease) 0.6s infinite;
  }
  @keyframes nudge-b {
    0%, 70%, 100% { transform: translateY(0); }
    80% { transform: translateY(2px); }
    90% { transform: translateY(0); }
  }
  @keyframes nudge-a {
    0%, 70%, 100% { transform: rotate(180deg) translateY(0); }
    80% { transform: rotate(180deg) translateY(2px); }
    90% { transform: rotate(180deg) translateY(0); }
  }
  .float.h .more {
    left: auto;
    top: 50%;
    padding: 0 5px;
    flex-direction: column;
    height: auto;
    width: 18px;
    padding: 5px 0 6px;
  }
  .float.h .more.a {
    left: 2px;
    transform: translate(-6px, -50%) scale(0.9);
  }
  .float.h .more.b {
    right: 2px;
    transform: translate(6px, -50%) scale(0.9);
  }
  .float.h .more.show {
    transform: translate(0, -50%) scale(1);
  }
  .float.h .more.a :global(svg) {
    transform: rotate(90deg);
    animation: none;
  }
  .float.h .more.b :global(svg) {
    transform: rotate(-90deg);
    animation: none;
  }

  /* Where in the list you are: a hairline rail, shown while scrolling or hovering. */
  .rail {
    position: absolute;
    top: 6px;
    bottom: 6px;
    right: -3px;
    width: 2px;
    border-radius: 2px;
    opacity: 0;
    transition: opacity 260ms var(--ease);
    pointer-events: none;
  }
  .view:hover .rail,
  .view.active .rail {
    opacity: 1;
  }
  .rail i {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(var(--at) * 100%);
    height: calc(var(--t) * 100%);
    border-radius: 2px;
    background: color-mix(in srgb, var(--accent) 70%, transparent);
    transition: top 160ms var(--ease);
  }
  .float.h .rail {
    top: auto;
    bottom: -3px;
    left: 6px;
    right: 6px;
    width: auto;
    height: 2px;
  }
  .float.h .rail i {
    top: 0;
    bottom: 0;
    height: auto;
    left: calc(var(--at) * 100%);
    width: calc(var(--t) * 100%);
    transition: left 160ms var(--ease);
  }

  /* ---- footer: open the main window ---- */
  .foot {
    flex: 0 0 var(--foot);
    display: grid;
    place-items: end center;
    padding-top: 5px;
    border-top: 1px solid var(--line);
    margin: 0 4px;
  }
  .float.h .foot {
    place-items: center end;
    padding: 0 0 0 5px;
    border-top: 0;
    border-left: 1px solid var(--line);
    margin: 4px 0;
  }
  .open {
    width: 100%;
    height: 26px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--text-2);
    transition: background var(--dur) var(--ease), color var(--dur) var(--ease), transform 120ms var(--ease);
  }
  .float.h .open {
    width: 26px;
    height: 100%;
  }
  .open:hover {
    background: var(--accent-softer);
    color: var(--accent);
  }
  .open:active {
    transform: scale(0.94);
  }

  /* ---- resize grips ---- */
  .rz {
    position: absolute;
    z-index: 2;
  }
  .corner {
    width: 16px;
    height: 16px;
  }
  .corner.nw {
    left: calc(-1 * var(--inset));
    top: calc(-1 * var(--inset));
    cursor: nwse-resize;
  }
  .corner.se {
    right: calc(-1 * var(--inset));
    bottom: calc(-1 * var(--inset));
    cursor: nwse-resize;
  }
  .corner.ne {
    right: calc(-1 * var(--inset));
    top: calc(-1 * var(--inset));
    cursor: nesw-resize;
  }
  .corner.sw {
    left: calc(-1 * var(--inset));
    bottom: calc(-1 * var(--inset));
    cursor: nesw-resize;
  }
  /* A short arc following the strip's own corner, shown on hover. */
  .corner i {
    position: absolute;
    width: 10px;
    height: 10px;
    border: 0 solid var(--text-3);
    opacity: 0;
    transition: opacity var(--dur) var(--ease), border-color var(--dur) var(--ease);
  }
  .corner.nw i {
    left: 3px;
    top: 3px;
    border-left-width: 2px;
    border-top-width: 2px;
    border-top-left-radius: 12px;
  }
  .corner.ne i {
    right: 3px;
    top: 3px;
    border-right-width: 2px;
    border-top-width: 2px;
    border-top-right-radius: 12px;
  }
  .corner.sw i {
    left: 3px;
    bottom: 3px;
    border-left-width: 2px;
    border-bottom-width: 2px;
    border-bottom-left-radius: 12px;
  }
  .corner.se i {
    right: 3px;
    bottom: 3px;
    border-right-width: 2px;
    border-bottom-width: 2px;
    border-bottom-right-radius: 12px;
  }
  .float:hover .corner i {
    opacity: 0.55;
  }
  .corner:hover i {
    opacity: 1 !important;
    border-color: var(--accent);
  }
  .edge {
    display: grid;
    place-items: center;
  }
  .edge.n,
  .edge.s {
    left: 16px;
    right: 16px;
    height: 7px;
    cursor: ns-resize;
  }
  .edge.n {
    top: calc(-1 * var(--inset));
  }
  .edge.s {
    bottom: calc(-1 * var(--inset));
  }
  .edge.w,
  .edge.e {
    top: 16px;
    bottom: 16px;
    width: 7px;
    cursor: ew-resize;
  }
  .edge.w {
    left: calc(-1 * var(--inset));
  }
  /* Docked: the ends are the whole height (the strip is as short as the taskbar). */
  .float.docked .edge.w,
  .float.docked .edge.e {
    top: 0;
    bottom: 0;
    width: 6px;
  }
  .edge.e {
    right: calc(-1 * var(--inset));
  }
  /* A short tick on the edge: accent on the ends ("a cell more or less"), quiet on the
     long sides ("size"). */
  .edge i {
    border-radius: 2px;
    background: var(--accent);
    opacity: 0;
    transition: opacity var(--dur) var(--ease);
  }
  .edge.n i,
  .edge.s i {
    width: 16px;
    height: 3px;
  }
  .edge.w i,
  .edge.e i {
    width: 3px;
    height: 16px;
  }
  .side i {
    background: var(--text-3);
  }
  .edge:hover i {
    opacity: 0.9;
  }

  @media (prefers-reduced-motion: reduce) {
    .more.show :global(svg) {
      animation: none !important;
    }
  }
  :global([data-motion='reduce']) .more.show :global(svg) {
    animation: none !important;
  }
</style>
