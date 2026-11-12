<script lang="ts">
  // One instance in the main pane: who it is, the one primary action, usage windows (countdown
  // first), and while it runs how much memory it uses. Editing stays in the side panel (F2).
  import { onDestroy } from 'svelte'
  import { api } from '../lib/api'
  import { copyText, edit, instanceMenu, primary, quitInstance, setHiddenInFloat } from '../lib/actions'
  import { ago, bytes } from '../lib/format'
  import { t } from '../lib/i18n.svelte'
  import { installationOf, run, ui, visibleOrgs } from '../lib/state.svelte'
  import type { Instance } from '../lib/types'
  import { isLaunching, needsText, usageErrorText } from '../lib/usage'
  import Icon from '../components/Icon.svelte'
  import StatusBadge from '../components/StatusBadge.svelte'
  import UsageWindow from '../components/UsageWindow.svelte'

  let { inst }: { inst: Instance } = $props()

  const install = $derived(installationOf(inst))
  const orgs = $derived(visibleOrgs(inst))
  let picked = $state<string | null>(null)
  const org = $derived(orgs.find((o) => o.orgUuid === picked) ?? orgs[0])
  const usage = $derived(inst.usage)
  const why = $derived(needsText(inst))
  const email = $derived(usage?.report?.email ?? '')

  // Memory is read only while this pane shows a running instance (no polling otherwise).
  let stats = $state<{ memory: number; processes: number } | null>(null)
  let timer: ReturnType<typeof setInterval> | undefined
  $effect(() => {
    const id = inst.id
    const running = inst.running
    clearInterval(timer)
    stats = null
    if (!running) return
    const load = () => api.instanceStats(id).then((x) => (stats = x)).catch(() => (stats = null))
    load()
    timer = setInterval(load, 5000)
  })
  onDestroy(() => clearInterval(timer))

  let size = $state<number | null>(null)
  $effect(() => {
    const id = inst.id
    size = null
    api.profileSize(id).then((n) => (size = n)).catch(() => (size = null))
  })
</script>

<div class="detail">
  <header class="head">
    <StatusBadge {inst} size={44} />
    <div class="who">
      <h1 class="truncate">{inst.alias}</h1>
      <div class="meta">
        <span>{install?.name}</span>
        {#if email}<span class="sep">·</span><span class="truncate">{email}</span>{/if}
        {#if org?.plan}<span class="plan">{org.plan}</span>{/if}
      </div>
    </div>
  </header>
  <div class="acts">
      <button class="btn primary" disabled={!install?.present || isLaunching(inst)} onclick={() => primary(inst)}>
        {#if inst.running}<Icon name="focus" size={14} /> {t('앞으로 가져오기')}{:else if isLaunching(inst)}{t('여는 중…')}{:else}<Icon name="play" size={11} /> {t('실행')}{/if}
      </button>
      {#if inst.running}
        <button class="btn" onclick={() => run(api.restart(inst.id))}><Icon name="refresh" size={14} /> {t('다시 시작')}</button>
        <button class="btn danger" onclick={() => quitInstance(inst)}><Icon name="power" size={14} /> {t('종료')}</button>
      {/if}
      <button class="btn" onclick={() => edit(inst)} title="F2"><Icon name="sliders" size={14} /> {t('편집')}</button>
      <button class="icon-btn" aria-label={t('더 보기')} onclick={(e) => instanceMenu(e, inst)}><Icon name="more" size={18} /></button>
  </div>

  {#if inst.advanced.hiddenInFloat}
    <div class="quiet-note">
      <Icon name="eye-off" size={15} />
      <span>{t('플로팅 바에서 숨겨 두었어요')}</span>
      <button class="btn sm" onclick={() => setHiddenInFloat(inst, false)}>{t('다시 표시')}</button>
    </div>
  {/if}

  {#if why}
    <div class="notice">
      <Icon name="alert" size={16} />
      <span>{why}{install?.present && install?.hasUsage ? ` — ${t('앱을 열어 로그인하면 사용량이 다시 보여요')}` : ''}</span>
      {#if install?.present}<button class="btn sm" onclick={() => primary(inst)}>{t('앱 열기')}</button>{/if}
    </div>
  {/if}

  {#if install?.hasUsage}
    <section>
      <div class="sec-head">
        <h2>{t('사용량')}</h2>
        {#if orgs.length > 1}
          <div class="orgs" role="tablist">
            {#each orgs as o (o.orgUuid)}
              <button role="tab" class:on={o.orgUuid === org?.orgUuid} aria-selected={o.orgUuid === org?.orgUuid} onclick={() => (picked = o.orgUuid)}>{o.plan || o.orgName}</button>
            {/each}
          </div>
        {/if}
        <span class="grow"></span>
        {#if usage?.report}<span class="faint small">{t('{time} 갱신', { time: ago(usage.report.fetchedAt, ui.now) })}</span>{/if}
        <button class="icon-btn sm" title={t('새로고침')} aria-label={t('새로고침')} disabled={!inst.effective.usageEnabled || usage?.status === 'loading'} onclick={() => run(api.refreshUsage(inst.id))}>
          <span class:spin={usage?.status === 'loading'}><Icon name="refresh" size={14} /></span>
        </button>
      </div>
      {#if !inst.effective.usageEnabled}
        <p class="faint">{t('이 인스턴스는 사용량 조회가 꺼져 있어요. 편집 → 고급에서 켤 수 있어요.')}</p>
      {:else if !usage?.report}
        {#if usage?.error}
          <p class="muted">{usageErrorText(usage.error)}</p>
        {:else}
          <div class="grid">{#each [0, 1] as i (i)}<div class="skel"></div>{/each}</div>
        {/if}
      {:else if org}
        <div class="grid">
          {#each org.quotas as q (q.kind + q.label)}<UsageWindow {q} />{/each}
        </div>
        {#if org.breakdown.length}
          <div class="split">
            <div class="faint small">{t('이번 주 사용 비중')}</div>
            <span class="splitbar">{#each org.breakdown.filter((b) => b.percent > 0) as b, i (b.label)}<span class="seg s{i}" style:width="{b.percent}%"></span>{/each}</span>
            <span class="legend">{#each org.breakdown as b, i (b.label)}<span><i class="s{i}"></i>{b.label} <span class="num">{Math.round(b.percent)}%</span></span>{/each}</span>
          </div>
        {/if}
        {#if usage.error}<p class="stale"><Icon name="alert" size={13} /> {usageErrorText(usage.error)} · {t('{time} 기준 숫자예요', { time: ago(usage.report.fetchedAt, ui.now) })}</p>{/if}
      {/if}
    </section>
  {/if}

  <section>
    <h2>{t('정보')}</h2>
    <dl class="facts">
      {#if inst.running}
        <dt>{t('메모리')}</dt>
        <dd class="num">{stats ? `${bytes(stats.memory)} · ${t('프로세스 {n}개', { n: stats.processes })}` : '…'}</dd>
      {/if}
      <dt>{t('마지막 실행')}</dt>
      <dd>{inst.lastLaunchedAt ? `${ago(inst.lastLaunchedAt, ui.now)} · ${t('{n}번 실행', { n: inst.launchCount })}` : t('런처로 실행한 적 없음')}</dd>
      <dt>{t('프로필 폴더')}</dt>
      <dd class="path">
        <span class="truncate" title={inst.profileDir ?? ''}>{inst.profile.kind === 'default' ? t('앱 기본 폴더') : (inst.profileDir ?? '')}</span>
        {#if size !== null}<span class="faint num">{bytes(size)}</span>{/if}
        <button class="icon-btn sm" title={t('열기')} aria-label={t('프로필 폴더 열기')} onclick={() => run(api.openProfile(inst.id))}><Icon name="folder" size={14} /></button>
        {#if inst.profileDir}<button class="icon-btn sm" title={t('경로 복사')} aria-label={t('경로 복사')} onclick={() => copyText(inst.profileDir!, t('경로를 복사했어요'))}><Icon name="copy" size={14} /></button>{/if}
      </dd>
      {#if install?.version}<dt>{t('앱 버전')}</dt><dd class="num">{install.version}</dd>{/if}
      {#if inst.note}<dt>{t('메모')}</dt><dd class="note">{inst.note}</dd>{/if}
    </dl>
  </section>
</div>

<style>
  .detail {
    max-width: 880px;
    margin: 0 auto;
    padding: 28px 32px 40px;
  }
  .head {
    --knock: var(--layer);
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .who {
    flex: 1;
    min-width: 0;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-22);
    font-weight: 650;
    letter-spacing: -0.015em;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    min-width: 0;
    color: var(--text-2);
    font-size: var(--fs-13);
  }
  .sep {
    color: var(--text-3);
  }
  .plan {
    flex: none;
    padding: 0 6px;
    border-radius: var(--r-xs);
    background: var(--s2);
    font-size: var(--fs-12);
    font-weight: 600;
  }
  .acts {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin-top: 16px;
    padding-left: 60px;
  }
  .quiet-note {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 16px;
    padding: 8px 8px 8px 12px;
    border-radius: var(--r-md);
    background: var(--surface-hover);
    color: var(--text-2);
    font-size: var(--fs-13);
  }
  .quiet-note span {
    flex: 1;
  }
  .notice {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 20px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: color-mix(in srgb, var(--dot-warn) 12%, transparent);
    color: var(--st-warn);
    font-size: var(--fs-13);
  }
  .notice span {
    flex: 1;
  }
  section {
    margin-top: 32px;
  }
  h2 {
    margin: 0 0 12px;
    font-size: var(--fs-15);
    font-weight: 650;
  }
  .sec-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }
  .sec-head h2 {
    margin: 0;
  }
  .grow {
    flex: 1;
  }
  .small {
    font-size: var(--fs-12);
  }
  .spin {
    display: grid;
    animation: spin 900ms linear infinite;
  }
  .orgs {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: var(--r-sm);
    background: var(--well);
  }
  .orgs button {
    border: 0;
    background: transparent;
    padding: 2px 10px;
    border-radius: var(--r-xs);
    font-size: var(--fs-12);
    color: var(--text-2);
  }
  .orgs button.on {
    background: var(--surface-solid);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 20px 32px;
  }
  .skel {
    height: 44px;
    border-radius: var(--r-sm);
    background: linear-gradient(90deg, var(--s2), var(--s1), var(--s2));
    background-size: 200% 100%;
    animation: sh 1.4s linear infinite;
  }
  .split {
    display: grid;
    gap: 6px;
    margin-top: 24px;
  }
  .splitbar {
    display: flex;
    height: 6px;
    border-radius: 3px;
    overflow: hidden;
    background: var(--track);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    font-size: var(--fs-12);
    color: var(--text-2);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .legend i {
    width: 8px;
    height: 8px;
    border-radius: 2px;
  }
  .s0 {
    background: var(--accent);
  }
  .s1 {
    background: var(--dot-warn);
  }
  .s2 {
    background: var(--dot-ok);
  }
  .s3 {
    background: #a78bfa;
  }
  .s4 {
    background: var(--s4);
  }
  .stale {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 14px 0 0;
    font-size: var(--fs-12);
    color: var(--st-warn);
  }
  .facts {
    display: grid;
    grid-template-columns: 120px 1fr;
    gap: 10px 16px;
    margin: 0;
    font-size: var(--fs-13);
  }
  dt {
    color: var(--text-2);
  }
  dd {
    margin: 0;
    min-width: 0;
  }
  .path {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .note {
    white-space: pre-wrap;
    word-break: keep-all;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes sh {
    to {
      background-position: -200% 0;
    }
  }
</style>
