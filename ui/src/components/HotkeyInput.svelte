<script lang="ts">
  import { t } from '../lib/i18n.svelte'
  // Records a key combination in the format the global-shortcut plugin parses ("Alt+Shift+Space").
  let { value, onchange }: { value: string; onchange: (v: string) => void } = $props()
  let recording = $state(false)

  const NAMES: Record<string, string> = { ' ': 'Space', ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right', Escape: 'Escape' }
  function key(e: KeyboardEvent) {
    if (!recording) return
    e.preventDefault()
    if (e.key === 'Escape') {
      recording = false
      return
    }
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return
    const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super'].filter(Boolean)
    if (!mods.length) return
    let k = NAMES[e.key] ?? (e.code.startsWith('Key') ? e.code.slice(3) : e.code.startsWith('Digit') ? e.code.slice(5) : e.key.length === 1 ? e.key.toUpperCase() : e.key)
    onchange([...mods, k].join('+'))
    recording = false
  }
</script>

<button class="hk" class:rec={recording} onclick={() => (recording = !recording)} onkeydown={key} onblur={() => (recording = false)}>
  {#if recording}{t('키를 누르세요…')}{:else}{#each value.split('+') as p, i (i)}<span class="kbd">{p}</span>{/each}{/if}
</button>

<style>
  .hk {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 30px;
    min-width: 150px;
    justify-content: center;
    padding: 0 10px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--well);
    font-size: var(--fs-12);
  }
  .hk.rec {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
    color: var(--accent);
  }
</style>
