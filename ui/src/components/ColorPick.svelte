<script lang="ts">
  import { t } from '../lib/i18n.svelte'
  const PALETTE = ['#0A84FF', '#FF9F0A', '#30D158', '#BF5AF2', '#FF375F', '#64D2FF', '#FFD60A', '#AC8E68', '#5E5CE6', '#FF6B3D']
  let { value, onchange }: { value: string; onchange: (v: string) => void } = $props()
</script>

<div class="swatches">
  {#each PALETTE as c (c)}
    <button class="sw" class:on={c.toLowerCase() === value.toLowerCase()} style:--c={c} aria-label={c} onclick={() => onchange(c)}></button>
  {/each}
  <label class="sw custom" title={t('직접 고르기')} style:--c={value}>
    <input type="color" {value} oninput={(e) => onchange((e.target as HTMLInputElement).value.toUpperCase())} />
  </label>
</div>

<style>
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .sw {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 0;
    padding: 0;
    background: var(--c);
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.12);
    transition: transform var(--dur) var(--ease);
  }
  .sw:hover {
    transform: scale(1.1);
  }
  .sw.on {
    box-shadow: 0 0 0 2px var(--surface-solid), 0 0 0 4px var(--c);
  }
  .custom {
    position: relative;
    overflow: hidden;
    background: conic-gradient(#ff375f, #ffd60a, #30d158, #64d2ff, #5e5ce6, #bf5af2, #ff375f);
  }
  .custom input {
    position: absolute;
    inset: 0;
    opacity: 0;
  }
</style>
