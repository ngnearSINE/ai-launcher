<script lang="ts">
  // Instance sidebar (Paseo-style): pinned rows, then sections by status / app / group, each
  // collapsible. A row shows the icon with its status badge, the alias and the least-left
  // percentage; on hover the trailing info gives way to the primary action and ⋯ (overlaid, so
  // nothing shifts). Holding Ctrl numbers the first nine rows for Ctrl+1…9.
  import { flip } from 'svelte/animate'
  import { api } from '../lib/api'
  import { instanceMenu, newInstance, primary } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { caps, keysOf } from '../lib/keymap.svelte'
  import { fold, sidebarOrder, sidebarSections, toggleFold, usageInstances } from '../lib/sidebar.svelte'
  import { createSortable, mergeOrder } from '../lib/sortable.svelte'
  import { dialogs, installationOf, run, settings, ui } from '../lib/state.svelte'
  import type { Instance, Settings as S } from '../lib/types'
  import { isLaunching, levelOf, longLabel, mainOrg, needsText, shortLeft, statusOf, windowsOf } from '../lib/usage'
  import Icon from './Icon.svelte'
  import StatusBadge from './StatusBadge.svelte'

  let { rail = false }: { rail?: boolean } = $props()

  const s = $derived(settings())
  const sections = $derived(sidebarSections())
  const order = $derived(sidebarOrder())
  const accounts = $derived(usageInstances())
  let listEl = $state<HTMLElement | null>(null)
  let displayOpen = $state(false)

  // Drag to reorder only where the list is one plain manual order.
  const sorter = createSortable({
    ids: () => order.map((i) => i.id),
    commit: (ids) => run(api.reorder(mergeOrder((ui.snap?.instances ?? []).map((i) => i.id), ids))),
    enabled: () => s.sort === 'manual' && s.sidebarGroup === 'none' && !rail,
    scroller: () => listEl,
  })
  const flipMs = $derived(s.reduceMotion ? 0 : 180)

  function select(inst: Instance) {
    ui.selected = inst.id
  }
  function set<K extends keyof S>(key: K, value: S[K]) {
    run(api.updateSettings({ [key]: value } as Partial<S>))
  }
  function rowTitle(inst: Instance) {
    const why = needsText(inst)
    return [inst.alias, inst.usage?.report?.email, why].filter(Boolean).join('\n')
  }
</script>

<svelte:window onmousedown={(e) => displayOpen && !(e.target as HTMLElement).closest('.display, .display-btn') && (displayOpen = false)} />

<aside class="sidebar" class:rail aria-label={t('인스턴스')}>
  <nav class="nav">
    <button class="nav-row" onclick={() => newInstance()} title={rail ? t('새 인스턴스') : undefined}>
      <Icon name="plus" size={16} />{#if !rail}<span>{t('새 인스턴스')}</span><span class="keys">{#each caps(keysOf('newInstance')) as c, k (k)}<span class="kbd">{c}</span>{/each}</span>{/if}
    </button>
    <button class="nav-row" class:on={!ui.selected} onclick={() => (ui.selected = null)} title={rail ? t('개요') : undefined}>
      <Icon name="home" size={16} />{#if !rail}<span>{t('개요')}</span>{/if}
    </button>
  </nav>

  <div class="list" bind:this={listEl} role="listbox" aria-label={t('인스턴스')}>
    {#each sections as sec (sec.id)}
      {#if !rail}
        <button class="sec" onclick={() => toggleFold(sec.id)} aria-expanded={!fold[sec.id]}>
          <span class="chev" class:closed={fold[sec.id]}><Icon name="chevron-down" size={12} /></span>
          {sec.label}
          <span class="n num">{sec.items.length}</span>
        </button>
      {:else}
        <div class="rail-sep"></div>
      {/if}
      {#if !fold[sec.id] || rail}
        {#each sorter.arrange(sec.items, (i) => i.id) as inst (inst.id)}
          {@const idx = order.indexOf(inst)}
          {@const w = windowsOf(mainOrg(inst), s)}
          {@const st = statusOf(inst, s)}
          <div
            class="row"
            class:on={ui.selected === inst.id}
            class:missing={!installationOf(inst)?.present}
            class:sort-placeholder={sorter.st.dragging === inst.id}
            data-sort-id={inst.id}
            role="option"
            tabindex="0"
            aria-selected={ui.selected === inst.id}
            title={rail ? rowTitle(inst) : (needsText(inst) ?? undefined)}
            animate:flip={{ duration: flipMs }}
            onpointerdown={(e) => sorter.down(e, inst.id)}
            onclick={() => select(inst)}
            ondblclick={() => primary(inst)}
            onkeydown={(e) => e.key === 'Enter' && primary(inst)}
            oncontextmenu={(e) => instanceMenu(e, inst)}
          >
            <StatusBadge {inst} size={rail ? 26 : 22} />
            {#if !rail}
              <span class="txt">
                <span class="alias truncate">{inst.alias}</span>
                {#if s.sidebarShowAccount}
                  {@const acct = inst.usage?.report?.email || installationOf(inst)?.name || ''}
                  {#if acct && acct !== inst.alias}<span class="acct truncate">{acct}</span>{/if}
                {/if}
              </span>
              <span class="trail">
                {#if ui.numbers && idx >= 0 && idx < 9}
                  <span class="kbd">{idx + 1}</span>
                {:else if isLaunching(inst)}
                  <span class="faint small">{t('여는 중')}</span>
                {:else if st === 'needs'}
                  <span class="small st-warn">{t('확인')}</span>
                {:else if (s.sidebarShow5h && w.w5) || (s.sidebarShowWeekly && w.ww)}
                  <span class="uses num">
                    {#if s.sidebarShow5h && w.w5}<span class="u {levelOf(w.w5, s)}" title={t('5시간')}><i>5h</i>{Math.round(w.w5.shown)}</span>{/if}
                    {#if s.sidebarShowWeekly && w.ww}<span class="u {levelOf(w.ww, s)}" title={longLabel(w.ww.q)}><i>{longLabel(w.ww.q, true)}</i>{Math.round(w.ww.shown)}</span>{/if}
                  </span>
                {/if}
              </span>
              <span class="hover">
                <button
                  class="mini"
                  title={inst.running ? t('앞으로 가져오기') : t('실행')}
                  aria-label={inst.running ? t('앞으로 가져오기') : t('실행')}
                  disabled={!installationOf(inst)?.present}
                  onclick={(e) => (e.stopPropagation(), primary(inst))}><Icon name={inst.running ? 'focus' : 'play'} size={inst.running ? 14 : 11} /></button
                >
                <button class="mini" aria-label={t('더 보기')} onclick={(e) => (e.stopPropagation(), instanceMenu(e, inst))}><Icon name="more" size={16} /></button>
              </span>
            {:else if ui.numbers && idx >= 0 && idx < 9}
              <span class="num-badge">{idx + 1}</span>
            {/if}
          </div>
        {/each}
      {/if}
    {:else}
      {#if !rail}
        <div class="empty">
          <p class="t">{t('아직 인스턴스가 없어요')}</p>
          <p class="d">{t('앱을 추가하면 계정별 인스턴스를 만들 수 있어요')}</p>
          <div class="btns">
            <button class="btn sm primary" onclick={() => (dialogs.addApp = true)}>{t('앱 추가')}</button>
            <button class="btn sm" onclick={() => run(api.rescan(), t('다시 찾았어요'))}>{t('다시 찾기')}</button>
          </div>
        </div>
      {/if}
    {/each}
  </div>

  <footer class="foot">
    {#if accounts.length && !rail}
      <!-- Quiet usage summary: least-left per account; opens the overview. -->
      <button class="usage" onclick={() => (ui.selected = null)} title={t('계정별 사용량 보기')}>
        {#each accounts.slice(0, 4) as a (a.id)}
          {@const u = shortLeft(a, s)}
          <span class="acc">
            <StatusBadge inst={a} size={14} />
            <span class="meter"><span class="f {u.level}" style:width={u.text || '0%'}></span></span>
            <span class="num v {u.level}">{u.text || '–'}</span>
          </span>
        {/each}
        {#if accounts.length > 4}<span class="more faint">+{accounts.length - 4}</span>{/if}
      </button>
    {/if}
    <div class="tools">
      <button class="icon-btn sm display-btn" class:active={displayOpen} title={t('표시')} aria-label={t('표시')} onclick={() => (displayOpen = !displayOpen)}><Icon name="display" size={16} /></button>
      <button class="icon-btn sm" title={t('단축키') + ' (Shift+?)'} aria-label={t('단축키')} onclick={() => (ui.help = true)}><Icon name="help" size={16} /></button>
    </div>
  </footer>

  {#if displayOpen}
    <div class="display" role="dialog" aria-label={t('표시')}>
      <div class="d-lab">{t('묶기')}</div>
      <div class="seg">
        {#each [['none', t('없음')], ['status', t('상태')], ['app', t('앱')], ['group', t('그룹')]] as [v, l] (v)}
          <button class:on={s.sidebarGroup === v} onclick={() => set('sidebarGroup', v as S['sidebarGroup'])}>{l}</button>
        {/each}
      </div>
      <div class="d-lab">{t('정렬')}</div>
      <select class="select" value={s.sort} onchange={(e) => set('sort', (e.target as HTMLSelectElement).value as S['sort'])}>
        <option value="manual">{t('직접 (끌어서 순서 바꾸기)')}</option>
        <option value="name">{t('이름순')}</option>
        <option value="recent">{t('최근 실행순')}</option>
        <option value="headroom">{t('사용량 여유순')}</option>
      </select>
      {#if s.sort === 'manual' && s.sidebarGroup !== 'none'}<p class="hint">{t('끌어서 순서를 바꾸려면 「묶기: 없음」으로 두세요')}</p>{/if}
      <div class="d-lab">{t('행에 보이기')}</div>
      <label class="chk"><input type="checkbox" checked={s.sidebarShowAccount} onchange={(e) => set('sidebarShowAccount', (e.target as HTMLInputElement).checked)} /> {t('계정')}</label>
      <label class="chk"><input type="checkbox" checked={s.sidebarShowWeekly} onchange={(e) => set('sidebarShowWeekly', (e.target as HTMLInputElement).checked)} /> {t('주간 사용량')}</label>
      <label class="chk"><input type="checkbox" checked={s.sidebarShow5h} onchange={(e) => set('sidebarShow5h', (e.target as HTMLInputElement).checked)} /> {t('5시간 사용량')}</label>
    </div>
  {/if}
</aside>

<style>
  .sidebar {
    --knock: var(--bg-solid);
    position: relative;
    width: var(--sidebar-w);
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .sidebar.rail {
    width: 60px;
  }
  .nav {
    display: grid;
    gap: 2px;
    padding: 4px 8px 6px;
  }
  .nav-row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    color: var(--text-2);
    font-size: var(--fs-13);
    text-align: left;
  }
  .rail .nav-row {
    justify-content: center;
    padding: 0;
  }
  .nav-row:hover {
    background: var(--hover);
    color: var(--text);
  }
  .nav-row.on {
    background: var(--selected);
    color: var(--text);
  }
  .nav-row span:first-of-type {
    flex: 1;
  }
  .keys {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity var(--dur) var(--ease);
  }
  .nav-row:hover .keys {
    opacity: 1;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 8px 8px;
    scrollbar-width: thin;
  }
  .sec {
    display: flex;
    align-items: center;
    gap: 4px;
    width: 100%;
    height: 28px;
    margin-top: 8px;
    padding: 0 6px;
    border: 0;
    background: none;
    color: var(--text-3);
    font-size: var(--fs-12);
    font-weight: 500;
    text-align: left;
  }
  .sec:hover {
    color: var(--text-2);
  }
  .sec .n {
    margin-left: auto;
    opacity: 0.8;
  }
  .chev {
    display: grid;
    transition: transform var(--dur) var(--ease);
  }
  .chev.closed {
    transform: rotate(-90deg);
  }
  .rail-sep {
    height: 1px;
    margin: 8px 10px;
    background: var(--line);
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: var(--row-h);
    padding: 4px 8px 4px 10px;
    margin-top: 2px;
    border-radius: var(--r-md);
    outline: none;
  }
  .rail .row {
    justify-content: center;
    padding: 6px 0;
  }
  .row:hover {
    background: var(--hover);
  }
  .row:active {
    background: var(--press);
  }
  .row.on {
    background: var(--selected);
  }
  .row:focus-visible {
    box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent) 60%, transparent);
  }
  .row.missing .alias {
    color: var(--text-3);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .alias {
    font-size: var(--fs-13);
    line-height: 18px;
  }
  .acct {
    font-size: var(--fs-12);
    line-height: 16px;
    color: var(--text-3);
  }
  .trail {
    flex: none;
    display: flex;
    align-items: center;
  }
  .small {
    font-size: var(--fs-12);
  }
  .st-warn {
    color: var(--st-warn);
  }
  /* "5h 93  주 44": small faint labels, numbers in the level's colour. */
  .uses {
    display: flex;
    gap: 8px;
    font-size: var(--fs-12);
    color: var(--text-2);
  }
  .u i {
    font-style: normal;
    margin-right: 2px;
    color: var(--text-3);
  }
  .u.warn {
    color: var(--st-warn);
  }
  .u.crit {
    color: var(--st-crit);
  }
  /* Hover actions sit over the trailing info on a fade, so the row never reflows. */
  .hover {
    position: absolute;
    right: 4px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    gap: 2px;
    padding-left: 18px;
    background: linear-gradient(to right, transparent, var(--row-bg, var(--bg-solid)) 16px);
    opacity: 0;
    pointer-events: none;
    transition: opacity var(--dur) var(--ease);
  }
  .row:hover .hover,
  .row:focus-within .hover {
    opacity: 1;
    pointer-events: auto;
  }
  .mini {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: var(--r-sm);
    background: var(--s2);
    color: var(--text-2);
  }
  .mini:hover {
    background: var(--s3);
    color: var(--text);
  }
  .mini:disabled {
    opacity: 0.4;
  }
  .num-badge {
    position: absolute;
    left: 4px;
    top: 2px;
    min-width: 16px;
    height: 16px;
    border-radius: 4px;
    background: var(--s3);
    font-size: var(--fs-12);
    line-height: 16px;
    text-align: center;
  }
  .empty {
    margin: 12px 4px;
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .empty .t {
    margin: 0 0 4px;
    font-weight: 600;
  }
  .empty .d {
    margin: 0 0 12px;
    font-size: var(--fs-12);
    color: var(--text-2);
  }
  .btns {
    display: flex;
    gap: 6px;
  }
  .foot {
    flex: none;
    padding: 6px 8px 8px;
    border-top: 1px solid var(--line);
  }
  .usage {
    width: 100%;
    display: flex;
    flex-wrap: wrap;
    gap: 6px 10px;
    padding: 6px 6px 8px;
    border: 0;
    border-radius: var(--r-sm);
    background: none;
    text-align: left;
  }
  .usage:hover {
    background: var(--hover);
  }
  .acc {
    --knock: var(--bg-solid);
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .meter {
    width: 22px;
    height: 4px;
    border-radius: 2px;
    background: var(--track);
    overflow: hidden;
  }
  .meter .f {
    display: block;
    height: 100%;
    background: var(--s4);
  }
  .meter .f.warn {
    background: var(--dot-warn);
  }
  .meter .f.crit {
    background: var(--dot-crit);
  }
  .v {
    font-size: var(--fs-12);
    color: var(--text-2);
  }
  .v.warn {
    color: var(--st-warn);
  }
  .v.crit {
    color: var(--st-crit);
  }
  .more {
    font-size: var(--fs-12);
  }
  .tools {
    display: flex;
    gap: 2px;
  }
  .rail .tools {
    flex-direction: column;
    align-items: center;
  }
  .icon-btn.active {
    color: var(--accent);
    background: var(--accent-softer);
  }
  .display {
    position: absolute;
    left: 8px;
    bottom: 48px;
    z-index: 40;
    width: 232px;
    padding: 12px;
    display: grid;
    gap: 6px;
    border-radius: var(--r-md);
    background: var(--surface-solid);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-pop);
    animation: pop 120ms var(--ease);
  }
  .rail .display {
    left: 64px;
    bottom: 8px;
  }
  .d-lab {
    margin-top: 4px;
    font-size: var(--fs-12);
    color: var(--text-3);
  }
  .seg {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 2px;
    padding: 2px;
    border-radius: var(--r-sm);
    background: var(--well);
  }
  .seg button {
    height: 26px;
    border: 0;
    border-radius: var(--r-xs);
    background: none;
    color: var(--text-2);
    font-size: var(--fs-12);
  }
  .seg button.on {
    background: var(--surface-solid);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }
  .display .select {
    height: 30px;
    font-size: var(--fs-12);
  }
  .hint {
    margin: 0;
    font-size: var(--fs-12);
    color: var(--text-3);
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-13);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
