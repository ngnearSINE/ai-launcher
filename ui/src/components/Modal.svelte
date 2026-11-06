<script lang="ts">
  import type { Snippet } from 'svelte'
  import Icon from './Icon.svelte'
  import { t } from '../lib/i18n.svelte'

  let { title, width = 460, onclose, children, footer }: { title: string; width?: number; onclose: () => void; children: Snippet; footer?: Snippet } = $props()

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.stopPropagation()
      onclose()
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title} style:width="{width}px">
    <header>
      <h2>{title}</h2>
      <button class="icon-btn sm" aria-label={t('닫기')} onclick={onclose}><Icon name="x" size={16} /></button>
    </header>
    <div class="body">{@render children()}</div>
    {#if footer}<footer>{@render footer()}</footer>{/if}
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: var(--scrim);
    animation: fade var(--dur) var(--ease);
  }
  .modal {
    max-width: calc(100vw - 32px);
    max-height: calc(100vh - 48px);
    display: flex;
    flex-direction: column;
    border-radius: var(--r-modal);
    background: var(--surface-solid);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-pop);
    animation: pop 200ms var(--ease);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 16px 6px 20px;
  }
  h2 {
    margin: 0;
    font-size: var(--fs-17);
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .body {
    padding: 8px 20px 16px;
    overflow: auto;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px 18px;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.98);
    }
  }
</style>
