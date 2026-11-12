<script lang="ts">
  // The main pane when nothing is selected: which Claude account to use now, every account's
  // usage side by side, and what is running. Nothing decorative — only numbers you act on.
  import { api } from '../lib/api'
  import { newInstance, primary } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { usageInstances } from '../lib/sidebar.svelte'
  import { dialogs, installationOf, run, settings, ui, visibleInstances } from '../lib/state.svelte'
  import type { Instance } from '../lib/types'
  import { bestClaude, mainOrg, scopedOf, usageErrorText, windowsOf } from '../lib/usage'
  import Icon from '../components/Icon.svelte'
  import StatusBadge from '../components/StatusBadge.svelte'
  import UsageWindow from '../components/UsageWindow.svelte'

  const s = $derived(settings())
  const all = $derived(visibleInstances())
  const accounts = $derived(usageInstances())
  const running = $derived(all.filter((i) => i.running))
  const best = $derived(accounts.length > 1 ? bestClaude(all, s) : undefined)
  // Per-model allowance shown as its own column (the floating bar keeps to 5-hour / weekly).
  const FABLE = /fable/i
  const hasFable = $derived(accounts.some((a) => scopedOf(mainOrg(a), s, FABLE)))
  const bestWins = $derived(best ? windowsOf(mainOrg(best), s) : null)

  function open(inst: Instance) {
    ui.selected = inst.id
  }
</script>

<div class="overview">
  <header>
    <h1>{t('개요')}</h1>
    <p class="muted">{t('인스턴스 {n}개 · 실행 중 {m}개', { n: all.length, m: running.length })}</p>
  </header>

  {#if !all.length}
    <div class="empty">
      <p class="t">{t('아직 인스턴스가 없어요')}</p>
      <p class="d">{t('설치된 앱을 찾지 못했어요. 앱을 추가하거나 다시 찾아보세요.')}</p>
      <div class="btns">
        <button class="btn primary" onclick={() => (dialogs.addApp = true)}>{t('앱 추가')}</button>
        <button class="btn" onclick={() => run(api.rescan(), t('다시 찾았어요'))}>{t('다시 찾기')}</button>
      </div>
    </div>
  {:else}
    {#if best && bestWins && (bestWins.w5 || bestWins.ww)}
      <section class="pick">
        <div class="pick-head">
          <StatusBadge inst={best} size={36} />
          <div class="who">
            <div class="lab">{t('지금 쓰기 좋은 Claude 계정')}</div>
            <div class="name truncate">{best.alias} <span class="faint">{best.usage?.report?.email ?? ''}</span></div>
          </div>
          <button class="btn primary" onclick={() => primary(best)}>
            {#if best.running}<Icon name="focus" size={14} /> {t('앞으로 가져오기')}{:else}<Icon name="play" size={11} /> {t('열기')}{/if}
          </button>
        </div>
        <div class="pair">
          {#if bestWins.w5}<UsageWindow q={bestWins.w5.q} />{/if}
          {#if bestWins.ww}<UsageWindow q={bestWins.ww.q} />{/if}
        </div>
      </section>
    {/if}

    {#if accounts.length}
      <section>
        <h2>{t('계정별 사용량')}</h2>
        <div class="table" class:fable={hasFable} role="table">
          {#each accounts as a (a.id)}
            {@const w = windowsOf(mainOrg(a), s)}
            {@const fable = scopedOf(mainOrg(a), s, FABLE)}
            <div class="tr" role="row" tabindex="0" onclick={() => open(a)} onkeydown={(e) => e.key === 'Enter' && open(a)}>
              <div class="acc" role="cell">
                <StatusBadge inst={a} size={24} />
                <div class="who">
                  <div class="name truncate">{a.alias}</div>
                  <div class="faint small truncate">{a.usage?.report?.email ?? installationOf(a)?.name}</div>
                </div>
              </div>
              {#if w.w5 || w.ww}
                <div role="cell">{#if w.w5}<UsageWindow q={w.w5.q} />{/if}</div>
                <div role="cell">{#if w.ww}<UsageWindow q={w.ww.q} />{/if}</div>
                {#if hasFable}<div role="cell">{#if fable}<UsageWindow q={fable.q} />{/if}</div>{/if}
              {:else}
                <div class="span2 muted small" class:span3={hasFable} role="cell">{a.usage?.error ? usageErrorText(a.usage.error) : t('불러오는 중…')}</div>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}

    <section>
      <h2>{t('실행 중')}</h2>
      {#if running.length}
        <div class="run-list">
          {#each running as r (r.id)}
            <div class="run-row">
              <StatusBadge inst={r} size={24} />
              <button class="link truncate" onclick={() => open(r)}>{r.alias}</button>
              <span class="faint small truncate">{installationOf(r)?.name}</span>
              <button class="btn sm" onclick={() => primary(r)}><Icon name="focus" size={13} /> {t('전환')}</button>
            </div>
          {/each}
        </div>
      {:else}
        <p class="faint">{t('실행 중인 인스턴스가 없어요. 왼쪽에서 고르거나 Ctrl+K로 찾아 실행하세요.')}</p>
      {/if}
    </section>

    <p class="foot faint">
      <button class="link" onclick={() => newInstance()}>{t('새 인스턴스')}</button>
      <span>·</span>
      <button class="link" onclick={() => (ui.palette = true)}>{t('명령 팔레트')} <span class="kbd">Ctrl</span><span class="kbd">K</span></button>
    </p>
  {/if}
</div>

<style>
  .overview {
    --knock: var(--layer);
    max-width: 960px;
    margin: 0 auto;
    padding: 28px 32px 40px;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-22);
    font-weight: 650;
    letter-spacing: -0.015em;
  }
  header p {
    margin: 4px 0 0;
    font-size: var(--fs-13);
  }
  section {
    margin-top: 32px;
  }
  h2 {
    margin: 0 0 12px;
    font-size: var(--fs-15);
    font-weight: 650;
  }
  .small {
    font-size: var(--fs-12);
  }
  .pick {
    padding: 16px 18px 18px;
    border-radius: var(--r-lg);
    background: var(--s1);
    border: 1px solid var(--line);
  }
  .pick-head {
    --knock: var(--s1);
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .who {
    flex: 1;
    min-width: 0;
  }
  .lab {
    font-size: var(--fs-12);
    color: var(--text-2);
  }
  .name {
    font-size: var(--fs-15);
    font-weight: 600;
  }
  .name .faint {
    font-size: var(--fs-13);
    font-weight: 400;
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 32px;
    margin-top: 16px;
  }
  .table {
    display: grid;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .tr {
    display: grid;
    grid-template-columns: minmax(180px, 1.1fr) 1fr 1fr;
    align-items: center;
    gap: 28px;
    padding: 12px 16px;
    outline: none;
  }
  .fable .tr {
    grid-template-columns: minmax(180px, 1.1fr) 1fr 1fr 1fr;
    gap: 24px;
  }
  .span3 {
    grid-column: span 3;
  }
  .tr + .tr {
    border-top: 1px solid var(--line);
  }
  .tr:hover {
    background: var(--hover);
  }
  .tr:focus-visible {
    box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent) 60%, transparent);
  }
  .tr .name {
    font-size: var(--fs-13);
  }
  .acc {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .span2 {
    grid-column: span 2;
  }
  .run-list {
    display: grid;
    gap: 2px;
  }
  .run-row {
    display: grid;
    grid-template-columns: auto auto 1fr auto;
    align-items: center;
    gap: 10px;
    padding: 6px 8px;
    border-radius: var(--r-md);
  }
  .run-row:hover {
    background: var(--hover);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--text);
    font-size: var(--fs-13);
    text-align: left;
  }
  .link:hover {
    color: var(--accent);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 40px;
    font-size: var(--fs-12);
  }
  .foot .link {
    font-size: var(--fs-12);
    color: var(--text-2);
  }
  .empty {
    margin-top: 24px;
    padding: 20px;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
  }
  .empty .t {
    margin: 0 0 4px;
    font-weight: 600;
  }
  .empty .d {
    margin: 0 0 14px;
    color: var(--text-2);
  }
  .btns {
    display: flex;
    gap: 8px;
  }
  @media (max-width: 720px) {
    .tr {
      grid-template-columns: 1fr;
      gap: 10px;
    }
    .span2,
    .span3 {
      grid-column: auto;
    }
    .fable .tr {
      grid-template-columns: 1fr;
      gap: 10px;
    }
    .pair {
      grid-template-columns: 1fr;
      gap: 14px;
    }
  }
</style>
