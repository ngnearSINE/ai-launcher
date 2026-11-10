<script lang="ts">
  // Toast and the confirm dialog, mounted once per window.
  import { dialogs, dismissToast, pauseToast, toastState } from '../lib/state.svelte'
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
  const ICON = { error: 'alert', warn: 'alert', ok: 'check', info: 'info' } as const
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

<!-- One toast at a time; a new one replaces it. Hover holds it. -->
<div class="region" aria-live="polite">
  {#if toastState.current}
    {@const x = toastState.current}
    {#key x.id}
      <div class="toast {x.kind}" role="status" onmouseenter={() => pauseToast(true)} onmouseleave={() => pauseToast(false)}>
        <span class="ic"><Icon name={ICON[x.kind]} size={16} /></span>
        <span class="txt">{x.text}</span>
        {#if x.action}
          {@const a = x.action}
          <button class="act" onclick={() => (a.run(), dismissToast())}>{a.label}</button>
        {/if}
        <button class="x" aria-label={t('닫기')} onclick={dismissToast}><Icon name="x" size={13} /></button>
      </div>
    {/key}
  {/if}
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
  .region {
    position: fixed;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    z-index: 80;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: min(480px, calc(100vw - 32px));
    padding: 8px 8px 8px 12px;
    border-radius: var(--r-md);
    background: var(--surface-solid);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-pop);
    font-size: var(--fs-13);
    animation: up 140ms var(--ease);
  }
  .ic {
    display: grid;
    color: var(--text-2);
  }
  .ok .ic {
    color: var(--st-ok);
  }
  .warn .ic {
    color: var(--st-warn);
  }
  .error .ic {
    color: var(--st-crit);
  }
  .txt {
    flex: 1;
    min-width: 0;
  }
  .act {
    border: 0;
    background: var(--accent-softer);
    color: var(--accent);
    font-weight: 600;
    height: 26px;
    padding: 0 10px;
    border-radius: var(--r-sm);
  }
  .act:hover {
    background: var(--accent-soft);
  }
  .x {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border: 0;
    border-radius: var(--r-xs);
    background: none;
    color: var(--text-3);
  }
  .x:hover {
    background: var(--surface-hover);
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
