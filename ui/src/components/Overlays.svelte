<script lang="ts">
  // Toasts and the confirm dialog, mounted once per window.
  import { dialogs, toasts } from '../lib/state.svelte'
  import { t } from '../lib/i18n.svelte'
  import Modal from './Modal.svelte'
  import ContextMenu from './ContextMenu.svelte'
  import Icon from './Icon.svelte'

  let checked = $state(false)
  $effect(() => {
    if (dialogs.confirm) checked = dialogs.confirm.checkbox?.value ?? false
  })
  function done(ok: boolean) {
    const c = dialogs.confirm
    dialogs.confirm = null
    c?.resolve({ ok, checked })
  }
</script>

<ContextMenu />

{#if dialogs.confirm}
  {@const c = dialogs.confirm}
  <Modal title={c.title} width={420} onclose={() => done(false)}>
    <p class="msg">{c.message}</p>
    {#if c.checkbox}
      <label class="check"><input type="checkbox" bind:checked /> {c.checkbox.label}</label>
    {/if}
    {#snippet footer()}
      <button class="btn" onclick={() => done(false)}>{t('취소')}</button>
      <!-- svelte-ignore a11y_autofocus -->
      <button class="btn primary" class:danger-fill={c.danger} autofocus onclick={() => done(true)}>{c.confirm}</button>
    {/snippet}
  </Modal>
{/if}

<div class="toasts" aria-live="polite">
  {#each toasts as x (x.id)}
    <div class="toast {x.kind}">
      <Icon name={x.kind === 'error' ? 'alert' : x.kind === 'ok' ? 'check' : 'info'} size={14} />
      <span>{x.text}</span>
    </div>
  {/each}
</div>

<style>
  .msg {
    margin: 0 0 12px;
    color: var(--text-2);
    line-height: 1.55;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-13);
  }
  :global(.btn.primary.danger-fill) {
    background: var(--crit);
  }
  .toasts {
    position: fixed;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    z-index: 80;
    display: grid;
    gap: 8px;
    justify-items: center;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 520px;
    padding: 9px 14px;
    border-radius: 10px;
    background: color-mix(in srgb, var(--surface-solid) 96%, transparent);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-pop);
    font-size: var(--fs-13);
    animation: up 180ms var(--ease);
  }
  .toast.error {
    color: var(--crit);
  }
  .toast.ok :global(svg) {
    color: var(--ok);
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }
</style>
