<script lang="ts">
  import type { Snippet } from 'svelte'
  // One setting: label + description on the left, control on the right.
  let { label, desc = '', children, stack = false }: { label: string; desc?: string; children: Snippet; stack?: boolean } = $props()
</script>

<div class="row" class:stack>
  <div class="txt">
    <div class="lb">{label}</div>
    {#if desc}<div class="ds">{desc}</div>{/if}
  </div>
  <div class="ctl">{@render children()}</div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row.stack {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }
  .txt {
    min-width: 0;
  }
  .lb {
    font-size: var(--fs-13);
    font-weight: 500;
  }
  .ds {
    font-size: var(--fs-12);
    color: var(--text-2);
    margin-top: 2px;
    line-height: 1.5;
  }
  .ctl {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .stack .ctl {
    flex: auto;
  }
</style>
