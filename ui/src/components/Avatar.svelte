<script lang="ts">
  import { ui, installationOf } from '../lib/state.svelte'
  import type { Instance } from '../lib/types'

  // App icon (read from the executable) or a brand tile, with the instance colour as a dot —
  // the same dot the taskbar badge shows, so cards and pinned buttons match.
  let { inst, size = 40, dot = true, ring = null }: { inst: Instance; size?: number; dot?: boolean; ring?: number | null } = $props()

  const install = $derived(installationOf(inst))
  const icon = $derived(install ? ui.icons[install.id] : undefined)
  const brand = $derived(install?.brand ?? { color: '#8E8E93', letter: '?', logo: 'generic' })
  const letter = $derived(inst.glyph || brand.letter || inst.alias.slice(0, 1))
  const r = $derived(size / 2 + 3)
  const c = $derived(2 * Math.PI * r)
</script>

<span class="av" style:--s="{size}px" style:--c={inst.color} style:--b={brand.color}>
  {#if icon}
    <img src={icon} alt="" draggable="false" />
  {:else if brand.logo === 'gemini'}
    <span class="tile gem">
      <svg viewBox="0 0 24 24" width={size * 0.56} height={size * 0.56}><path d="M12 2c.6 5.3 4.7 9.4 10 10-5.3.6-9.4 4.7-10 10-.6-5.3-4.7-9.4-10-10 5.3-.6 9.4-4.7 10-10z" fill="currentColor" /></svg>
    </span>
  {:else}
    <span class="tile" style:font-size="{size * 0.42}px">{letter}</span>
  {/if}
  {#if ring !== null}
    <svg class="ring" width={size + 8} height={size + 8} viewBox="0 0 {size + 8} {size + 8}">
      <circle cx={size / 2 + 4} cy={size / 2 + 4} {r} class="track" />
      <circle
        cx={size / 2 + 4}
        cy={size / 2 + 4}
        {r}
        class="val"
        class:warn={ring >= 70}
        class:crit={ring >= 90}
        stroke-dasharray="{(c * Math.min(100, ring)) / 100} {c}"
      />
    </svg>
  {/if}
  {#if dot}<span class="dot" class:run={inst.running}></span>{/if}
</span>

<style>
  .av {
    position: relative;
    display: inline-grid;
    place-items: center;
    width: var(--s);
    height: var(--s);
    flex: none;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 1px 1.5px rgba(0, 0, 0, 0.18));
  }
  .tile {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    border-radius: 24%;
    background: linear-gradient(160deg, color-mix(in srgb, var(--b) 88%, white), var(--b));
    color: white;
    font-weight: 700;
    letter-spacing: -0.02em;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.18);
  }
  .tile.gem {
    background: linear-gradient(140deg, #5b8def, #9b72cb 55%, #d96570);
  }
  .dot {
    position: absolute;
    right: -2px;
    bottom: -2px;
    width: max(9px, calc(var(--s) * 0.3));
    height: max(9px, calc(var(--s) * 0.3));
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 2px var(--surface-solid);
  }
  .ring {
    position: absolute;
    inset: -4px;
    transform: rotate(-90deg);
    pointer-events: none;
  }
  .ring circle {
    fill: none;
    stroke-width: 2.5;
  }
  .ring .track {
    stroke: var(--track);
  }
  .ring .val {
    stroke: var(--accent);
    stroke-linecap: round;
    transition: stroke-dasharray 400ms var(--ease);
  }
  .ring .val.warn {
    stroke: var(--warn);
  }
  .ring .val.crit {
    stroke: var(--crit);
  }
</style>
