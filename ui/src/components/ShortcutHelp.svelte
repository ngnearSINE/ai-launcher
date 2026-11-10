<script lang="ts">
  // Shift+? — every shortcut, searchable, grouped by section. Reads the same table the key
  // handler uses, so it always shows the user's current keys.
  import { openSettings } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { BINDINGS, caps, keysOf, SECTIONS } from '../lib/keymap.svelte'
  import { settings, ui } from '../lib/state.svelte'
  import Modal from './Modal.svelte'

  let q = $state('')
  const s = $derived(settings())
  const globals = $derived([
    { label: t('빠른 실행 열기'), keys: s.quickHotkeyEnabled ? s.quickHotkey : '' },
    { label: t('런처 창 ⇄ 플로팅'), keys: s.floatHotkeyEnabled ? s.floatHotkey : '' },
    { label: t('가장 여유 있는 Claude 실행'), keys: s.bestClaudeHotkeyEnabled ? s.bestClaudeHotkey : '' },
  ])
  const groups = $derived.by(() => {
    const needle = q.trim().toLowerCase()
    const hit = (label: string, keys: string) => !needle || label.toLowerCase().includes(needle) || keys.toLowerCase().includes(needle)
    const out = SECTIONS.map((sec) => ({
      label: sec.label(),
      rows: BINDINGS.filter((b) => b.section === sec.id)
        .map((b) => ({ label: b.label(), keys: keysOf(b.id) }))
        .filter((r) => hit(r.label, r.keys)),
    }))
    out.push({ label: t('어디서든 (전역)'), rows: globals.filter((r) => hit(r.label, r.keys)) })
    return out.filter((g) => g.rows.length)
  })
  function close() {
    ui.help = false
  }
</script>

<Modal title={t('단축키')} width={520} onclose={close}>
  <!-- svelte-ignore a11y_autofocus -->
  <input class="input" placeholder={t('단축키 검색')} bind:value={q} autofocus />
  {#each groups as g (g.label)}
    <h3>{g.label}</h3>
    <div class="box">
      {#each g.rows as r (r.label)}
        <div class="row">
          <span>{r.label}</span>
          <span class="keys">
            {#if r.keys}
              {#each r.keys.split(' / ') as alt, ai (ai)}
                {#if ai > 0}<span class="or faint">{t('또는')}</span>{/if}
                {#each caps(alt) as c, k (k)}<span class="kbd">{c}</span>{/each}
              {/each}
            {:else}
              <span class="faint">{t('지정 안 됨')}</span>
            {/if}
          </span>
        </div>
      {/each}
    </div>
  {:else}
    <p class="faint none">{t('맞는 단축키가 없어요')}</p>
  {/each}
  {#snippet footer()}
    <button
      class="btn"
      onclick={() => {
        close()
        openSettings('shortcuts')
      }}>{t('단축키 바꾸기…')}</button
    >
    <button class="btn primary" onclick={close}>{t('닫기')}</button>
  {/snippet}
</Modal>

<style>
  h3 {
    margin: 18px 0 6px;
    font-size: var(--fs-12);
    font-weight: 600;
    color: var(--text-3);
  }
  .box {
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 40px;
    padding: 6px 12px 6px 16px;
    font-size: var(--fs-13);
  }
  .row + .row {
    border-top: 1px solid var(--line);
  }
  .keys {
    display: flex;
    align-items: center;
    gap: 3px;
    flex: none;
  }
  .or {
    margin: 0 4px;
    font-size: var(--fs-12);
  }
  .none {
    margin: 24px 0;
    text-align: center;
  }
</style>
