<script lang="ts">
  import { t } from '../lib/i18n.svelte'
  import { caps, comboOf } from '../lib/keymap.svelte'
  // Records a key combination. Global hotkeys use the format the global-shortcut plugin parses
  // ("Alt+Shift+Space") and need a modifier. `app` records in-window shortcuts by physical key
  // (lib/keymap), where a few keys may stand alone (F1–F12, Enter, Delete …).
  let { value, onchange, app = false }: { value: string; onchange: (v: string) => void; app?: boolean } = $props()
  let recording = $state(false)

  const NAMES: Record<string, string> = { ' ': 'Space', ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right', Escape: 'Escape' }
  const BARE = /^(F\d{1,2}|Enter|Delete|Insert|Home|End|PageUp|PageDown|Backspace)$/
  function key(e: KeyboardEvent) {
    if (!recording) return
    e.preventDefault()
    e.stopPropagation()
    if (e.key === 'Escape') {
      recording = false
      return
    }
    if (['Control', 'Shift', 'Alt', 'Meta'].includes(e.key)) return
    const hasMod = e.ctrlKey || e.altKey || e.shiftKey || e.metaKey
    if (app) {
      const combo = comboOf(e)
      if (!combo || (!hasMod && !BARE.test(combo))) return
      onchange(combo)
    } else {
      const mods = [e.ctrlKey && 'Ctrl', e.altKey && 'Alt', e.shiftKey && 'Shift', e.metaKey && 'Super'].filter(Boolean)
      if (!mods.length) return
      const k = NAMES[e.key] ?? (e.code.startsWith('Key') ? e.code.slice(3) : e.code.startsWith('Digit') ? e.code.slice(5) : e.key.length === 1 ? e.key.toUpperCase() : e.key)
      onchange([...mods, k].join('+'))
    }
    recording = false
  }
  const parts = $derived(app ? caps(value) : value ? value.split('+') : [])
</script>

<button class="hk" class:rec={recording} onclick={() => (recording = !recording)} onkeydown={key} onblur={() => (recording = false)}>
  {#if recording}{t('키를 누르세요…')}{:else if parts.length}{#each parts as p, i (i)}<span class="kbd">{p}</span>{/each}{:else}<span class="faint">{t('지정 안 됨')}</span>{/if}
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
