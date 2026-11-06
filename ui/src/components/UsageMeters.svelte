<script lang="ts">
  import { t } from '../lib/i18n.svelte'
  import { until, ago, pct, money } from '../lib/format'
  import { settings, ui, visibleOrgs } from '../lib/state.svelte'
  import type { Instance, Quota, UsageError } from '../lib/types'
  import { win, levelOf } from '../lib/usage'
  import Icon from './Icon.svelte'

  // Usage of one instance, read from that instance's own profile. Several organisations
  // (team / personal) of the signed-in account appear as tabs.
  let { inst, compact = false, detail = false }: { inst: Instance; compact?: boolean; detail?: boolean } = $props()

  const usage = $derived(inst.usage)
  const orgs = $derived(visibleOrgs(inst))
  let picked = $state<string | null>(null)
  const org = $derived(orgs.find((o) => o.orgUuid === picked) ?? orgs[0])
  const s = $derived(settings())

  function label(q: Quota) {
    if (q.kind === 'fiveHour') return t('5시간')
    if (q.kind === 'weekly') return t('주간')
    if (q.kind === 'credits') return t('크레딧')
    return q.label
  }
  function errorLine(e: UsageError | null): string {
    if (!e) return ''
    switch (e.code) {
      case 'notSignedIn':
        return t('로그인하면 사용량이 보여요')
      case 'sessionExpired':
        return t('세션이 만료됐어요 · 앱을 열면 갱신돼요')
      case 'accountUnknown':
        return t('이 프로필의 계정을 확인할 수 없어요')
      case 'decrypt':
        return t('로그인 정보를 읽지 못했어요')
      case 'rateLimited':
        return t('요청 제한 · {n}분 뒤 다시 시도', { n: Math.ceil(e.detail / 60) })
      case 'network':
        return t('네트워크 오류')
      default:
        return t('사용량을 불러오지 못했어요')
    }
  }
  const quotas = $derived((org?.quotas ?? []).filter((q) => !compact || q.kind === 'fiveHour' || q.kind === 'weekly'))
</script>

{#if !inst.effective.usageEnabled}
  <div class="empty faint">{t('사용량 표시 꺼짐')}</div>
{:else if !usage || (usage.status === 'loading' && !usage.report) || usage.status === 'idle'}
  <div class="rows">
    {#each [0, 1] as i (i)}<div class="skel"></div>{/each}
  </div>
{:else if !usage.report}
  <div class="empty muted"><Icon name="info" size={14} /> {errorLine(usage.error)}</div>
{:else}
  {#if orgs.length > 1 && !compact}
    <div class="orgs" role="tablist">
      {#each orgs as o (o.orgUuid)}
        <button role="tab" class:on={o.orgUuid === org?.orgUuid} aria-selected={o.orgUuid === org?.orgUuid} onclick={(e) => { e.stopPropagation(); picked = o.orgUuid }}>
          {o.plan || o.orgName}
        </button>
      {/each}
    </div>
  {/if}
  <div class="rows">
    {#each quotas as q (q.kind + q.label)}
      {@const w = win(q, s, ui.now)}
      <div
        class="row {levelOf(w, s)}"
        title={[q.resetsAt ? t('{time} 후 초기화', { time: until(q.resetsAt, ui.now) }) : '', w.paceAt !== null ? t('흰 눈금: 고르게 썼을 때의 위치') : '', w.paceWarn ? t('지금 속도면 초기화 전에 바닥나요') : ''].filter(Boolean).join(' · ')}
      >
        <span class="lab">{label(q)}</span>
        <span class="bar">
          <span class="fill" style:width="{Math.min(100, w.shown)}%"></span>
          {#if w.paceAt !== null && q.kind !== 'credits'}<span class="pace" style:left="{w.paceAt}%"></span>{/if}
        </span>
        <span class="val num">{pct(w.shown)}</span>
        {#if !compact && s.usageShowReset}
          <span class="reset num faint">{q.resetsAt ? until(q.resetsAt, ui.now) : q.kind === 'credits' && q.limitAmount ? `${money(q.usedAmount ?? 0, q.currency)} / ${money(q.limitAmount, q.currency)}` : ''}</span>
        {/if}
      </div>
    {/each}
    {#if !quotas.length}<div class="empty faint">{t('표시할 한도가 없어요')}</div>{/if}
  </div>
  {#if detail && org?.breakdown.length}
    <div class="split" title={t('이번 주 사용 비중')}>
      <span class="splitbar">
        {#each org.breakdown.filter((b) => b.percent > 0) as b, i (b.label)}
          <span class="seg s{i}" style:width="{b.percent}%"></span>
        {/each}
      </span>
      <span class="legend faint">
        {#each org.breakdown as b, i (b.label)}<span><i class="s{i}"></i>{b.label} {Math.round(b.percent)}%</span>{/each}
      </span>
    </div>
  {/if}
  {#if usage.error && !compact}
    <div class="stale faint" title={errorLine(usage.error)}><Icon name="alert" size={12} /> {t('{time} 기준', { time: ago(usage.report.fetchedAt, ui.now) })}</div>
  {/if}
{/if}

<style>
  .rows {
    display: grid;
    gap: 7px;
  }
  .row {
    display: grid;
    grid-template-columns: 44px 1fr 38px auto;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-12);
  }
  .lab {
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar {
    position: relative;
    height: 6px;
    border-radius: 6px;
    background: var(--track);
  }
  .pace {
    position: absolute;
    top: -2px;
    width: 2px;
    height: 10px;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--tick);
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 6px;
    background: var(--accent);
    transition: width 500ms var(--ease);
  }
  .warn .fill {
    background: var(--warn);
  }
  .crit .fill {
    background: var(--crit);
  }
  .crit .val {
    color: var(--crit);
  }
  .val {
    text-align: right;
    font-weight: 600;
  }
  .reset {
    font-size: var(--fs-11);
    min-width: 58px;
    text-align: right;
  }
  .skel {
    height: 12px;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--track), var(--surface-2), var(--track));
    background-size: 200% 100%;
    animation: sh 1.4s linear infinite;
  }
  @keyframes sh {
    to {
      background-position: -200% 0;
    }
  }
  .empty {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-12);
    min-height: 20px;
  }
  .orgs {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border-radius: 7px;
    background: var(--well);
    margin-bottom: 8px;
  }
  .orgs button {
    border: 0;
    background: transparent;
    padding: 2px 8px;
    border-radius: 5px;
    font-size: var(--fs-11);
    color: var(--text-2);
  }
  .orgs button.on {
    background: var(--surface-solid);
    color: var(--text);
    box-shadow: var(--shadow-card);
  }
  .split {
    margin-top: 12px;
    display: grid;
    gap: 6px;
  }
  .splitbar {
    display: flex;
    height: 6px;
    border-radius: 6px;
    overflow: hidden;
    background: var(--track);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: var(--fs-11);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .legend i {
    width: 7px;
    height: 7px;
    border-radius: 2px;
  }
  .s0 {
    background: var(--accent);
  }
  .s1 {
    background: #ff9f0a;
  }
  .s2 {
    background: #30d158;
  }
  .s3 {
    background: #bf5af2;
  }
  .s4 {
    background: #8e8e93;
  }
  .stale {
    margin-top: 6px;
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--fs-11);
  }
</style>
