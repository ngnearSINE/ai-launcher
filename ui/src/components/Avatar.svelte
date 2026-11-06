<script lang="ts">
  import { ui, installationOf } from '../lib/state.svelte'
  import type { Instance } from '../lib/types'
  import { RING, ringBox } from '../lib/usage'

  // App icon (read from the executable / web app) or a brand tile. Optional concentric usage
  // rings, outermost first — same reading as the AI Usage widget: the arc is the value shown
  // (left by default), the white tick is the even-pace position.
  interface Ring {
    value: number
    paceAt: number | null
    level: '' | 'warn' | 'crit'
  }
  let { inst, size = 40, rings = [] }: { inst: Instance; size?: number; rings?: Ring[] } = $props()

  const install = $derived(installationOf(inst))
  const icon = $derived(install ? ui.icons[install.id] : undefined)
  const brand = $derived(install?.brand ?? { color: '#8E8E93', letter: '?', logo: 'generic' })
  const letter = $derived(inst.glyph || brand.letter || inst.alias.slice(0, 1))

  // Ring geometry in a 100×100 box: 270° arcs opening at the bottom, like the widget.
  // The box grows with the number of rings so the innermost one never touches the icon.
  const A0 = Math.PI * 0.75
  const SWEEP = Math.PI * 1.5
  const stroke = RING.stroke
  const gap = RING.gap
  const box = $derived(ringBox(size, rings.length))
  function arc(r: number, a0: number, a1: number) {
    const p = (a: number) => [50 + r * Math.cos(a), 50 + r * Math.sin(a)]
    const [x0, y0] = p(a0)
    const [x1, y1] = p(a1)
    return `M${x0.toFixed(2)} ${y0.toFixed(2)}A${r} ${r} 0 ${a1 - a0 > Math.PI ? 1 : 0} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`
  }
  const radius = (i: number) => 50 - RING.margin - stroke / 2 - i * (stroke + gap)
</script>

<span class="av" style:--s="{size}px" style:--box="{box}px" style:--b={brand.color}>
  {#if rings.length}
    <svg class="rings" viewBox="0 0 100 100" aria-hidden="true">
      {#each rings as r, i (i)}
        {@const rad = radius(i)}
        {@const end = A0 + SWEEP * Math.max(0.001, Math.min(100, r.value) / 100)}
        <path d={arc(rad, A0, A0 + SWEEP)} class="track" stroke-width={stroke} />
        <path d={arc(rad, A0, end)} class="val {r.level}" class:inner={i > 0} stroke-width={stroke} />
        {#if r.paceAt !== null}
          {@const pa = A0 + (SWEEP * Math.max(0, Math.min(100, r.paceAt))) / 100}
          <line
            x1={50 + (rad - stroke * 0.8) * Math.cos(pa)}
            y1={50 + (rad - stroke * 0.8) * Math.sin(pa)}
            x2={50 + (rad + stroke * 0.8) * Math.cos(pa)}
            y2={50 + (rad + stroke * 0.8) * Math.sin(pa)}
            class="tick"
          />
        {/if}
      {/each}
    </svg>
  {/if}
  <span class="face">
    {#if icon}
      <img src={icon} alt="" draggable="false" />
    {:else if brand.logo === 'gemini'}
      <span class="tile gem">
        <svg viewBox="0 0 24 24" width={size * 0.56} height={size * 0.56}><path d="M12 2c.6 5.3 4.7 9.4 10 10-5.3.6-9.4 4.7-10 10-.6-5.3-4.7-9.4-10-10 5.3-.6 9.4-4.7 10-10z" fill="currentColor" /></svg>
      </span>
    {:else}
      <span class="tile" style:font-size="{size * 0.42}px">{letter}</span>
    {/if}
  </span>
</span>

<style>
  .av {
    position: relative;
    display: inline-grid;
    place-items: center;
    width: var(--box);
    height: var(--box);
    flex: none;
  }
  /* Icon below, rings always on top (the icon's drop-shadow filter would otherwise paint over them). */
  .face {
    position: relative;
    z-index: 1;
    width: var(--s);
    height: var(--s);
    display: grid;
    place-items: center;
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
  .rings {
    position: absolute;
    z-index: 2;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    overflow: visible;
  }
  .rings path {
    fill: none;
    stroke-linecap: round;
  }
  .track {
    stroke: var(--track);
  }
  .val {
    stroke: var(--ring, var(--accent));
    transition: d 400ms var(--ease);
  }
  .val.inner {
    stroke: color-mix(in srgb, var(--ring, var(--accent)) 62%, var(--surface-solid));
  }
  .val.warn {
    --ring: var(--warn);
  }
  .val.crit {
    --ring: var(--crit);
  }
  .tick {
    stroke: var(--tick);
    stroke-width: 1.6;
    stroke-linecap: round;
  }
</style>
