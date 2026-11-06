<script lang="ts" generics="T extends string">
  let { value, options, onchange, size = 'md' }: { value: T; options: { value: T; label: string }[]; onchange: (v: T) => void; size?: 'sm' | 'md' } = $props()
</script>

<div class="seg {size}" role="radiogroup">
  {#each options as o (o.value)}
    <button role="radio" aria-checked={o.value === value} class:on={o.value === value} onclick={() => onchange(o.value)}>{o.label}</button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    border-radius: 8px;
    background: var(--well);
  }
  button {
    border: 0;
    background: transparent;
    height: 26px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: var(--fs-12);
    color: var(--text-2);
    white-space: nowrap;
    transition: background var(--dur) var(--ease), color var(--dur) var(--ease);
  }
  .sm button {
    height: 22px;
    padding: 0 8px;
    font-size: var(--fs-11);
  }
  button.on {
    background: var(--surface-solid);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }
</style>
