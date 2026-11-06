<script lang="ts">
  import { onMount } from 'svelte'
  import { boot, ui } from './lib/state.svelte'
  import Overlays from './components/Overlays.svelte'
  import MainView from './views/MainView.svelte'
  import QuickView from './views/QuickView.svelte'
  import FloatView from './views/FloatView.svelte'

  let error = $state('')
  onMount(() => {
    boot().catch((e) => (error = String(e)))
    // Native context menu stays off; the app draws its own.
    const block = (e: MouseEvent) => {
      if (!(e.target as HTMLElement).closest('input, textarea')) e.preventDefault()
    }
    addEventListener('contextmenu', block)
    return () => removeEventListener('contextmenu', block)
  })
</script>

{#if error}
  <pre class="err">{error}</pre>
{:else if ui.snap}
  {#if ui.label === 'quick'}
    <QuickView />
  {:else if ui.label === 'float'}
    <FloatView />
  {:else}
    <MainView />
  {/if}
  <Overlays />
{/if}

<style>
  .err {
    padding: 20px;
    color: var(--crit);
    white-space: pre-wrap;
  }
</style>
