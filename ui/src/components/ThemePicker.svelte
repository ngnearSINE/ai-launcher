<script lang="ts">
  // Theme tiles, each a tiny sketch of the app in that theme (sidebar, rows, a usage bar), so
  // the choice is seen rather than read. "System" is split light | dark. Ctrl+Alt+T cycles.
  import { THEME_LABEL } from '../lib/commands.svelte'
  import { THEMES, type ThemeId } from '../lib/state.svelte'

  let { value, accent, onchange }: { value: string; accent: string; onchange: (v: ThemeId) => void } = $props()

  // Mirrors tokens.css (sidebar, content, row, text, own accent if the theme has one).
  const P: Record<Exclude<ThemeId, 'system'>, { side: string; pane: string; row: string; fg: string; accent?: string }> = {
    light: { side: '#f1f1f3', pane: '#ffffff', row: '#e4e4e7', fg: '#18181b' },
    dark: { side: '#151517', pane: '#1b1b1d', row: '#3f3f43', fg: '#f4f4f5' },
    claude: { side: '#1f1f1e', pane: '#262624', row: '#4a4946', fg: '#f5f4ef', accent: '#d97757' },
    midnight: { side: '#0d121d', pane: '#121826', row: '#343d52', fg: '#eef2f9' },
    black: { side: '#000000', pane: '#000000', row: '#2b2b2d', fg: '#f5f5f5' },
  }
</script>

{#snippet sketch(p: { side: string; pane: string; row: string; fg: string; accent?: string })}
  <span class="app" style:--side={p.side} style:--pane={p.pane} style:--row={p.row} style:--fg={p.fg} style:--acc={p.accent ?? accent}>
    <span class="side"><i></i><i class="on"></i><i></i><i></i></span>
    <span class="pane"><i class="h"></i><i class="bar"><b></b></i><i class="bar"><b class="w"></b></i></span>
  </span>
{/snippet}

<div class="grid" role="radiogroup">
  {#each THEMES as th (th)}
    <button class="tile" class:on={value === th} role="radio" aria-checked={value === th} onclick={() => onchange(th)}>
      <span class="frame">
        {#if th === 'system'}
          <span class="half l">{@render sketch(P.light)}</span>
          <span class="half r">{@render sketch(P.dark)}</span>
        {:else}
          {@render sketch(P[th])}
        {/if}
      </span>
      <span class="name">{THEME_LABEL[th]()}</span>
    </button>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    width: 100%;
    max-width: 520px;
  }
  .tile {
    display: grid;
    gap: 6px;
    padding: 6px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: transparent;
    text-align: left;
  }
  .tile:hover {
    background: var(--hover);
  }
  .tile.on {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .name {
    padding: 0 2px 2px;
    font-size: var(--fs-12);
  }
  .frame {
    position: relative;
    display: block;
    aspect-ratio: 16 / 9;
    border-radius: var(--r-sm);
    overflow: hidden;
  }
  .half {
    position: absolute;
    inset: 0;
  }
  .half.l {
    clip-path: inset(0 50% 0 0);
  }
  .half.r {
    clip-path: inset(0 0 0 50%);
  }
  .app {
    position: absolute;
    inset: 0;
    display: flex;
    background: var(--pane);
  }
  .side {
    width: 32%;
    background: var(--side);
    display: grid;
    align-content: start;
    gap: 3px;
    padding: 8px 5px;
  }
  .side i {
    height: 4px;
    border-radius: 2px;
    background: var(--row);
  }
  .side i.on {
    background: color-mix(in srgb, var(--fg) 30%, var(--side));
  }
  .pane {
    flex: 1;
    display: grid;
    align-content: start;
    gap: 5px;
    padding: 9px 8px;
  }
  .pane .h {
    width: 46%;
    height: 5px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--fg) 75%, var(--pane));
  }
  .pane .bar {
    height: 4px;
    border-radius: 2px;
    background: var(--row);
  }
  .pane .bar b {
    display: block;
    width: 62%;
    height: 100%;
    border-radius: 2px;
    background: var(--acc);
  }
  .pane .bar b.w {
    width: 34%;
  }
</style>
