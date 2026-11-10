<script lang="ts">
  // App icon with one status badge on its bottom-right corner (Paseo's project badge): a fixed
  // 12px frame whose content changes — warning glyph when the instance needs the user, a dot
  // for running low, a ring while running (spinning while it starts). A knockout rim in the
  // surface colour (--knock) keeps it legible over any icon.
  import { settings } from '../lib/state.svelte'
  import type { Instance } from '../lib/types'
  import { isLaunching, statusOf } from '../lib/usage'
  import Avatar from './Avatar.svelte'

  let { inst, size = 24, rings = [] }: { inst: Instance; size?: number; rings?: { value: number; paceAt: number | null; level: '' | 'warn' | 'crit' }[] } = $props()

  const status = $derived(statusOf(inst, settings()))
  const starting = $derived(isLaunching(inst))
  // Running shows as a ring around the centre dot; the dot carries the usage level.
  const ring = $derived(inst.running || starting)
  const dot = $derived(status === 'crit' ? 'crit' : status === 'warn' ? 'warn' : ring ? 'run' : '')
</script>

<span class="wrap" style:--sz="{size}px">
  <Avatar {inst} {size} {rings} />
  {#if status === 'needs'}
    <span class="badge needs" aria-hidden="true">
      <svg viewBox="0 0 12 12"><path d="M6 1.2 11.2 10.6H.8z" /><path class="mark" d="M6 4.6v2.6M6 8.9v.01" /></svg>
    </span>
  {:else if dot}
    <span class="badge" class:spin={starting} aria-hidden="true">
      {#if ring}<i class="ring"></i>{/if}
      <i class="dot {dot}"></i>
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
    width: 6px;
    height: 6px;
    border-radius: 50%;
    grid-area: 1 / 1;
  }
  .dot.run {
    background: var(--dot-run);
  }
  .dot.warn {
    background: var(--dot-warn);
  }
  .dot.crit {
    background: var(--dot-crit);
  }
  .ring {
    grid-area: 1 / 1;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1.5px solid color-mix(in srgb, var(--dot-run) 75%, transparent);
  }
  /* Starting: a quarter arc runs round the ring (900ms, as in Paseo) until the process shows. */
  .spin .ring {
    border-color: color-mix(in srgb, var(--dot-run) 28%, transparent);
    border-top-color: var(--dot-run);
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
