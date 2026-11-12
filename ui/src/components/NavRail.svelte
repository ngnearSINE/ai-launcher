<script lang="ts">
  // The main window's page rail: the instances on top (where the window opens), the settings
  // people come back to under them, and general / data / about pinned to the foot. Icon over a
  // short label, so every page is one click away at any window width.
  import { t } from '../lib/i18n.svelte'
  import { caps, keysOf } from '../lib/keymap.svelte'
  import { PAGES } from '../lib/pages'
  import { goPage, ui } from '../lib/state.svelte'
  import Icon from './Icon.svelte'

  const groups = $derived([PAGES.filter((p) => p.group === 'top'), PAGES.filter((p) => p.group === 'main'), PAGES.filter((p) => p.group === 'end')])
  const settingsKeys = $derived(caps(keysOf('settings')).join('+'))
</script>

<nav class="rail" aria-label={t('메뉴')}>
  {#each groups as list, g (g)}
    {#if g === 1}<span class="sep" aria-hidden="true"></span>{/if}
    {#if g === 2}<span class="grow" aria-hidden="true"></span>{/if}
    {#each list as p (p.id)}
      <button
        class="item"
        class:on={ui.page === p.id}
        aria-current={ui.page === p.id ? 'page' : undefined}
        title={p.id === 'instances' ? p.label() : `${p.label()}${settingsKeys ? ` · ${t('설정')} ${settingsKeys}` : ''}`}
        onclick={() => goPage(p.id)}
      >
        <Icon name={p.icon} size={19} />
        <span class="lab">{p.label()}</span>
      </button>
    {/each}
  {/each}
</nav>

<style>
  .rail {
    flex: none;
    width: 76px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 6px 6px 10px;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .rail::-webkit-scrollbar {
    display: none;
  }
  .item {
    flex: none;
    width: 64px;
    min-height: 52px;
    padding: 7px 2px 6px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    border: 0;
    border-radius: var(--r-md);
    background: transparent;
    color: var(--text-2);
    transition:
      background var(--dur) var(--ease),
      color var(--dur) var(--ease);
  }
  .item:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .item.on {
    background: var(--selected);
    color: var(--text);
  }
  .item.on :global(svg) {
    color: var(--accent);
  }
  .lab {
    font-size: var(--fs-12);
    line-height: 1.2;
    text-align: center;
    word-break: keep-all;
  }
  .item.on .lab {
    font-weight: 600;
  }
  .item:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .sep {
    flex: none;
    width: 28px;
    height: 1px;
    margin: 6px 0;
    background: var(--line);
  }
  .grow {
    flex: 1;
    min-height: 12px;
  }
</style>
