<script lang="ts">
  import { tick } from 'svelte'
  import { menu } from '../lib/state.svelte'
  import Icon from './Icon.svelte'

  let el = $state<HTMLDivElement | null>(null)
  let pos = $state({ x: 0, y: 0 })
  let active = $state(-1)

  $effect(() => {
    if (!menu.open) return
    active = -1
    pos = { x: menu.x, y: menu.y }
    tick().then(() => {
      if (!el) return
      const r = el.getBoundingClientRect()
      pos = {
        x: Math.max(6, Math.min(menu.x, innerWidth - r.width - 6)),
        y: Math.max(6, Math.min(menu.y, innerHeight - r.height - 6)),
      }
      el.focus()
    })
  })

  const actionable = $derived(menu.items.map((it, i) => (!it.sep && !it.disabled ? i : -1)).filter((i) => i >= 0))

  function close() {
    menu.open = false
  }
  function pick(i: number) {
    const it = menu.items[i]
    if (!it || it.sep || it.disabled) return
    close()
    it.action?.()
  }
  function key(e: KeyboardEvent) {
    if (!menu.open) return
    if (e.key === 'Escape') {
      e.preventDefault()
      close()
    } else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault()
      const at = actionable.indexOf(active)
      const n = actionable.length
      active = actionable[(at + (e.key === 'ArrowDown' ? 1 : n - 1) + (at < 0 && e.key === 'ArrowUp' ? 1 : 0)) % n]
    } else if (e.key === 'Enter' && active >= 0) {
      e.preventDefault()
      pick(active)
    }
  }
</script>

<svelte:window onkeydown={key} onblur={close} onresize={close} />

{#if menu.open}
  <div class="catcher" role="presentation" onmousedown={close} oncontextmenu={(e) => { e.preventDefault(); close() }}></div>
  <div class="menu" bind:this={el} tabindex="-1" role="menu" style:left="{pos.x}px" style:top="{pos.y}px">
    {#each menu.items as it, i (i)}
      {#if it.sep}
        <div class="sep" role="separator"></div>
      {:else}
        <button
          role="menuitem"
          class:danger={it.danger}
          class:active={i === active}
          disabled={it.disabled}
          onmouseenter={() => (active = i)}
          onclick={() => pick(i)}
        >
          <span class="ic">{#if it.icon}<Icon name={it.icon} size={14} />{/if}</span>
          <span class="lb">{it.label}</span>
          {#if it.hint}<span class="hint">{it.hint}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .catcher {
    position: fixed;
    inset: 0;
    z-index: 60;
  }
  .menu {
    position: fixed;
    z-index: 61;
    min-width: 200px;
    padding: 5px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-solid) 94%, transparent);
    backdrop-filter: blur(20px) saturate(1.6);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-pop);
    outline: none;
    animation: in 120ms var(--ease);
  }
  button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    font-size: var(--fs-13);
    text-align: left;
  }
  button.active {
    background: var(--accent);
    color: var(--accent-contrast);
  }
  button.active .hint {
    color: inherit;
    opacity: 0.8;
  }
  button:disabled {
    opacity: 0.38;
  }
  button.danger:not(.active) {
    color: var(--crit);
  }
  .ic {
    width: 16px;
    display: inline-grid;
    place-items: center;
    opacity: 0.85;
  }
  .lb {
    flex: 1;
  }
  .hint {
    font-size: var(--fs-11);
    color: var(--text-3);
  }
  .sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--line);
  }
  @keyframes in {
    from {
      opacity: 0;
      transform: scale(0.97);
    }
  }
</style>
