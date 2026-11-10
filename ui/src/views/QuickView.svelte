<script lang="ts">
  // Tray / hotkey quick panel: the command palette filling its own small window. Same commands
  // and ranking as Ctrl+K in the main window; Esc or losing focus hides it.
  import { listen } from '@tauri-apps/api/event'
  import { onMount } from 'svelte'
  import { api } from '../lib/api'
  import CommandPalette from '../components/CommandPalette.svelte'

  let palette = $state<ReturnType<typeof CommandPalette> | null>(null)

  onMount(() => {
    const un = listen('quick-opened', () => palette?.reset())
    return () => {
      un.then((f) => f())
    }
  })
</script>

<div class="quick">
  <CommandPalette bind:this={palette} mode="panel" onclose={() => api.hideWindow('quick')} />
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
  }
  .quick {
    position: absolute;
    inset: 0;
  }
</style>
