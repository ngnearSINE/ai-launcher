<script lang="ts">
  // App icon with one badge on its bottom-right corner, the same corner the taskbar icon of a
  // split instance carries: a dot in the instance's own colour, so two accounts of one app are
  // told apart everywhere alike (sidebar, floating bar, taskbar). A ring round the dot says
  // "running" (spinning while it starts) and turns amber / red when usage runs low; a warning
  // glyph replaces the dot when the instance needs the user. A knockout rim in the surface
  // colour (--knock) keeps it legible over any icon.
  import { settings } from '../lib/state.svelte'
  import type { Instance } from '../lib/types'
  import { isLaunching, statusOf } from '../lib/usage'
  import Avatar from './Avatar.svelte'

  interface Ring {
    value: number
    paceAt: number | null
    level: '' | 'warn' | 'crit'
    empty?: boolean
  }
  let { inst, size = 24, rings = [], badge = true }: { inst: Instance; size?: number; rings?: Ring[]; badge?: boolean } = $props()

  const status = $derived(statusOf(inst, settings()))
  const starting = $derived(isLaunching(inst))
  const level = $derived(status === 'crit' || status === 'warn' ? status : '')
  const ring = $derived(inst.running || starting || !!level)
</script>

<span class="wrap" style:--sz="{size}px">
  <Avatar {inst} {size} {rings} />
  {#if !badge}
    <!-- the caller shows state its own way -->
  {:else if status === 'needs'}
    <span class="badge needs" aria-hidden="true">
      <svg viewBox="0 0 12 12"><path d="M6 1.2 11.2 10.6H.8z" /><path class="mark" d="M6 4.6v2.6M6 8.9v.01" /></svg>
    </span>
  {:else}
    <span class="badge {level}" class:spin={starting} aria-hidden="true">
      {#if ring}<i class="ring"></i>{/if}
      <i class="dot" style:--c={inst.color}></i>
    </span>
  {/if}
</span>

<style>
  .wrap {
    position: relative;
    display: inline-grid;
    flex: none;
  }
  .badge {
    position: absolute;
    right: -4px;
    bottom: -4px;
    z-index: 3;
    width: 14px;
    height: 14px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--knock, var(--bg-solid));
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    grid-area: 1 / 1;
    background: var(--c);
    box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.18);
  }
  .ring {
    --rc: color-mix(in srgb, var(--text-2) 80%, transparent);
    grid-area: 1 / 1;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1.5px solid var(--rc);
  }
  .warn .ring {
    --rc: var(--dot-warn);
  }
  .crit .ring {
    --rc: var(--dot-crit);
  }
  /* Starting: a quarter arc runs round the ring (900ms, as in Paseo) until the process shows. */
  .spin .ring {
    border-color: color-mix(in srgb, var(--rc) 30%, transparent);
    border-top-color: var(--rc);
    animation: spin 900ms linear infinite;
  }
  .needs svg {
    width: 12px;
    height: 12px;
  }
  .needs path {
    fill: var(--dot-warn);
    stroke: var(--dot-warn);
    stroke-width: 1;
    stroke-linejoin: round;
  }
  .needs .mark {
    fill: none;
    stroke: var(--bg-solid);
    stroke-width: 1.4;
    stroke-linecap: round;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spin .ring {
      animation-duration: 2.4s;
    }
  }
</style>
