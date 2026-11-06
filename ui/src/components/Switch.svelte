<script lang="ts">
  let { checked, onchange, label = '', disabled = false }: { checked: boolean; onchange: (v: boolean) => void; label?: string; disabled?: boolean } = $props()
</script>

<button class="sw" class:on={checked} role="switch" aria-checked={checked} aria-label={label} {disabled} onclick={() => onchange(!checked)}>
  <span class="knob"></span>
</button>

<style>
  .sw {
    position: relative;
    width: 38px;
    height: 22px;
    flex: none;
    border-radius: 22px;
    border: 0;
    padding: 0;
    background: var(--track);
    transition: background var(--dur) var(--ease);
  }
  .sw.on {
    background: var(--accent);
  }
  .sw:disabled {
    opacity: 0.4;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: white;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
    transition: transform var(--dur) var(--ease);
  }
  .on .knob {
    transform: translateX(16px);
  }
</style>
