<script lang="ts">
  // Command palette. The same component is the Ctrl+K overlay in the main window and the whole
  // tray / hotkey quick panel ("panel"). Type to filter, ↑↓ to move, Enter to run, Esc to clear
  // then close; in the panel, 1–9 launch the first instances while nothing is typed.
  import { tick } from 'svelte'
  import { api } from '../lib/api'
  import { instanceMenu } from '../lib/actions'
  import { commands, rank, type Hit } from '../lib/commands.svelte'
  import { t } from '../lib/i18n.svelte'
  import { caps } from '../lib/keymap.svelte'
  import { settings } from '../lib/state.svelte'
  import { shortLeft } from '../lib/usage'
  import Icon from './Icon.svelte'
  import StatusBadge from './StatusBadge.svelte'

  let { mode = 'overlay', onclose }: { mode?: 'overlay' | 'panel'; onclose: () => void } = $props()

  let query = $state('')
  let active = $state(0)
  let input = $state<HTMLInputElement | null>(null)
  let listEl = $state<HTMLElement | null>(null)

  const sections = $derived(rank(query, commands(mode === 'panel' ? 'quick' : 'main')))
  const flat = $derived(sections.flatMap((s) => s.hits))
  const instHits = $derived(flat.filter((h) => h.cmd.group === 'instances'))

  $effect(() => {
    void query
    active = 0
  })

  /** Called by the quick window each time it opens. */
  export async function reset() {
    query = ''
    active = 0
    await tick()
    input?.focus()
  }
  $effect(() => {
    input?.focus()
  })

  async function exec(h: Hit | undefined) {
    if (!h) return
    onclose()
    await h.cmd.run()
  }
  function move(d: number) {
    if (!flat.length) return
    active = (active + d + flat.length) % flat.length
    tick().then(() => listEl?.querySelector(`[data-i="${active}"]`)?.scrollIntoView({ block: 'nearest' }))
  }
  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      move(1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      move(-1)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      exec(flat[active])
    } else if (e.key === 'Escape') {
      e.preventDefault()
      e.stopPropagation()
      if (query) query = ''
      else onclose()
    } else if (/^Digit[1-9]$/.test(e.code) && (e.ctrlKey || (mode === 'panel' && !query && !e.altKey))) {
      const h = instHits[Number(e.code.slice(5)) - 1]
      if (h) {
        e.preventDefault()
        exec(h)
      }
    }
  }

  /** Title split into plain and highlighted pieces. */
  function pieces(h: Hit): { text: string; hit: boolean }[] {
    const title = h.cmd.title
    if (!h.marks.length) return [{ text: title, hit: false }]
    const ranges = [...h.marks].sort((a, b) => a[0] - b[0])
    const out: { text: string; hit: boolean }[] = []
    let at = 0
    for (const [s0, e0] of ranges) {
      const s1 = Math.max(s0, at)
      if (s1 > at) out.push({ text: title.slice(at, s1), hit: false })
      if (e0 > s1) out.push({ text: title.slice(s1, e0), hit: true })
      at = Math.max(at, e0)
    }
    if (at < title.length) out.push({ text: title.slice(at), hit: false })
    return out
  }
  const indexOf = (h: Hit) => flat.indexOf(h)
</script>

{#snippet body()}
  <div class="search">
    <Icon name="search" size={16} />
    <input
      bind:this={input}
      bind:value={query}
      placeholder={mode === 'panel' ? t('실행할 인스턴스나 명령…') : t('인스턴스나 명령 검색…')}
      onkeydown={key}
      spellcheck="false"
      aria-label={t('명령 팔레트')}
      aria-controls="palette-list"
    />
    {#if mode === 'overlay'}<span class="kbd">Esc</span>{/if}
  </div>
  <div class="list" id="palette-list" role="listbox" bind:this={listEl}>
    {#each sections as sec (sec.group)}
      <div class="sec">{sec.label}</div>
      {#each sec.hits as h (h.cmd.id)}
        {@const i = indexOf(h)}
        {@const n = h.cmd.group === 'instances' ? instHits.indexOf(h) : -1}
        <button
          class="item"
          class:on={i === active}
          class:danger={h.cmd.danger}
          data-i={i}
          role="option"
          aria-selected={i === active}
          onmousemove={() => (active = i)}
          onclick={() => exec(h)}
          oncontextmenu={(e) => h.cmd.inst && instanceMenu(e, h.cmd.inst, mode === 'panel')}
        >
          <span class="lead">
            {#if h.cmd.inst}<StatusBadge inst={h.cmd.inst} size={22} />{:else}<Icon name={h.cmd.icon ?? 'bolt'} size={16} />{/if}
          </span>
          <span class="txt">
            <span class="title truncate">{#each pieces(h) as p, k (k)}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}</span>
            {#if h.cmd.subtitle}<span class="sub truncate">{h.cmd.subtitle}</span>{/if}
          </span>
          <span class="trail">
            {#if h.cmd.checked}<Icon name="check" size={15} />{/if}
            {#if h.cmd.group === 'instances' && h.cmd.inst}
              {@const u = shortLeft(h.cmd.inst, settings())}
              {#if u.text}<span class="pct num {u.level}">{u.text}</span>{/if}
              {#if n >= 0 && n < 9}<span class="kbd">{mode === 'panel' && !query ? n + 1 : `Ctrl ${n + 1}`}</span>{/if}
            {:else if h.cmd.shortcut}
              {#each caps(h.cmd.shortcut) as c, k (k)}<span class="kbd">{c}</span>{/each}
            {/if}
          </span>
        </button>
      {/each}
    {:else}
      <div class="none">
        <p>{t('맞는 항목이 없어요')}</p>
        <button class="btn sm" onclick={() => (query = '')}>{t('검색어 지우기')}</button>
      </div>
    {/each}
  </div>
  <div class="foot faint">
    <span><span class="kbd">↑</span><span class="kbd">↓</span> {t('이동')}</span>
    <span><span class="kbd">Enter</span> {t('실행')}</span>
    <span><span class="kbd">Esc</span> {query ? t('지우기') : t('닫기')}</span>
    {#if mode === 'panel'}
      <button class="link" onclick={() => (onclose(), api.showWindow('main'))}>{t('런처 창 열기')}</button>
    {/if}
  </div>
{/snippet}

{#if mode === 'overlay'}
  <div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
    <div class="palette" role="dialog" aria-modal="true" aria-label={t('명령 팔레트')}>{@render body()}</div>
  </div>
{:else}
  <div class="palette panel" role="dialog" aria-label={t('빠른 실행')}>{@render body()}</div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: var(--scrim);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 48px;
    animation: fade 120ms var(--ease);
  }
  .palette {
    --knock: var(--surface-solid);
    width: min(640px, calc(100vw - 32px));
    max-height: min(560px, 80vh);
    display: flex;
    flex-direction: column;
    border-radius: var(--r-md);
    background: var(--surface-solid);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-pop);
    overflow: hidden;
    animation: pop 140ms var(--ease);
  }
  .palette.panel {
    width: 100%;
    max-height: none;
    height: 100%;
    border-radius: var(--r-lg);
    animation: none;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 48px;
    padding: 0 14px;
    border-bottom: 1px solid var(--line);
    color: var(--text-3);
    flex: none;
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: 0;
    background: none;
    font-size: var(--fs-15);
    color: var(--text);
  }
  .list {
    flex: 1;
    overflow: auto;
    padding: 4px 6px 8px;
  }
  .sec {
    padding: 10px 8px 4px;
    font-size: var(--fs-12);
    color: var(--text-3);
  }
  .sec:not(:first-child) {
    margin-top: 4px;
    border-top: 1px solid var(--line);
  }
  .item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    padding: 5px 8px;
    border: 0;
    border-radius: var(--r-sm);
    background: transparent;
    text-align: left;
  }
  .item.on {
    background: var(--selected);
  }
  .item.danger .title {
    color: var(--st-crit);
  }
  .lead {
    width: 24px;
    display: grid;
    place-items: center;
    flex: none;
    color: var(--text-2);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .title {
    font-size: var(--fs-13);
    line-height: 18px;
  }
  mark {
    background: none;
    color: var(--accent);
    font-weight: 600;
  }
  .sub {
    font-size: var(--fs-12);
    line-height: 16px;
    color: var(--text-3);
  }
  .trail {
    display: flex;
    align-items: center;
    gap: 4px;
    flex: none;
    color: var(--accent);
  }
  .pct {
    font-size: var(--fs-12);
    color: var(--text-2);
    margin-right: 4px;
  }
  .pct.warn {
    color: var(--st-warn);
  }
  .pct.crit {
    color: var(--st-crit);
  }
  .none {
    padding: 28px 12px;
    display: grid;
    justify-items: center;
    gap: 8px;
    color: var(--text-2);
  }
  .none p {
    margin: 0;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 14px;
    height: 34px;
    padding: 0 14px;
    border-top: 1px solid var(--line);
    font-size: var(--fs-12);
    flex: none;
  }
  .foot .kbd {
    margin-right: 2px;
  }
  .link {
    margin-left: auto;
    border: 0;
    background: none;
    color: var(--accent);
    font-size: var(--fs-12);
    padding: 0;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.99);
    }
  }
</style>
