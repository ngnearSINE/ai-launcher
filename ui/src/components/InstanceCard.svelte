<script lang="ts">
  import { api } from '../lib/api'
  import { instanceMenu, launch, edit } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { ago } from '../lib/format'
  import { installationOf, run, settings, ui } from '../lib/state.svelte'
  import { mainOrg, paceWarned, severity } from '../lib/usage'
  import type { Instance } from '../lib/types'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import UsageMeters from './UsageMeters.svelte'

  let { inst, list = false, index = 0 }: { inst: Instance; list?: boolean; index?: number } = $props()

  const install = $derived(installationOf(inst))
  const org = $derived(mainOrg(inst))
  const email = $derived(inst.usage?.report?.email ?? '')
  const plan = $derived(org?.plan ?? '')
  const missing = $derived(!install?.present)
  const sev = $derived(inst.effective.usageEnabled ? severity(org, settings()) : '')
  const paceHot = $derived(!!sev && paceWarned(org, settings()))
  const webHost = $derived.by(() => {
    if (install?.adapter !== 'webapp') return ''
    try {
      return new URL(inst.launch.url || String(install.options.url ?? '')).host.replace(/^www\./, '')
    } catch {
      return ''
    }
  })
  // Who this copy is: the signed-in account when known, else what the app is.
  const identity = $derived(
    email || [install?.name !== inst.alias ? install?.name : '', webHost].filter(Boolean).join(' · ') || install?.name || '',
  )

  function primary() {
    if (missing) return
    if (inst.running) run(api.focus(inst.id))
    else launch(inst)
  }
  function onkey(e: KeyboardEvent) {
    if (e.key === 'Enter') primary()
    if (e.key === 'F2') edit(inst)
  }
</script>

<div
  class="card {sev}"
  class:list
  class:missing
  role="button"
  tabindex="0"
  data-index={index}
  onclick={primary}
  onkeydown={onkey}
  oncontextmenu={(e) => instanceMenu(e, inst)}
>
  <div class="head">
    <Avatar {inst} size={list ? 30 : 38} />
    <div class="title">
      <div class="alias">
        <i class="tag" style:background={inst.color} title={t('인스턴스 색')}></i>
        <span class="truncate">{inst.alias}</span>
        {#if plan}<span class="plan">{plan}</span>{/if}
        {#if inst.favorite}<span class="fav"><Icon name="star" size={12} /></span>{/if}
      </div>
      <div class="who truncate" class:email={!!email} title={identity}>{identity}</div>
    </div>
    {#if list}
      <div class="state">
        {#if sev}<span class="pill {sev}"><i></i>{paceHot ? t('속도 주의') : t('여유 부족')}</span>{/if}
        {#if inst.running}<span class="pill run" title={t('실행 중')}><i></i></span>{/if}
      </div>
    {/if}
    <button
      class="icon-btn sm more"
      aria-label={t('더 보기')}
      onclick={(e) => {
        e.stopPropagation()
        instanceMenu(e, inst)
      }}><Icon name="more" size={18} /></button
    >
  </div>

  {#if install?.hasUsage}
    <div class="usage">
      <UsageMeters {inst} compact={list || ui.snap?.settings.cardUsageStyle === 'compact'} />
    </div>
  {/if}

  {#if !list}
    <div class="foot">
      <span class="state">
        {#if missing}
          <span class="pill warn">{t('앱 없음')}</span>
        {:else}
          {#if inst.running}<span class="pill run"><i></i>{t('실행 중')}</span>{/if}
          {#if sev}<span class="pill {sev}"><i></i>{paceHot ? t('속도 주의') : t('여유 부족')}</span>{/if}
        {/if}
      </span>
      <span class="meta truncate">
        {#if inst.note}{inst.note}{:else if inst.lastLaunchedAt}{t('마지막 실행 {time}', { time: ago(inst.lastLaunchedAt, ui.now) })}{:else}{t('런처로 실행한 적 없음')}{/if}
      </span>
      <button
        class="btn sm act"
        class:primary={!inst.running}
        class:switch={inst.running}
        disabled={missing}
        title={inst.running ? t('이 인스턴스 창으로 전환') : t('실행')}
        onclick={(e) => {
          e.stopPropagation()
          primary()
        }}
      >
        {#if inst.running}<Icon name="focus" size={13} /> {t('전환')}{:else}<Icon name="play" size={11} /> {t('실행')}{/if}
      </button>
    </div>
  {/if}
</div>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    height: 100%;
    padding: 14px 14px 12px 16px;
    border-radius: var(--r-card);
    background: var(--surface);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-card);
    outline: none;
    overflow: hidden;
    transition: transform var(--dur) var(--ease), box-shadow var(--dur) var(--ease), border-color var(--dur) var(--ease);
  }
  /* Warning stripe on the left edge (as in AI Usage), clipped by the card's rounded corners. */
  .card.warn {
    --sev: var(--warn);
  }
  .card.crit {
    --sev: var(--crit);
  }
  .card.warn,
  .card.crit {
    border-color: color-mix(in srgb, var(--sev) 45%, var(--line));
  }
  .card.warn::before,
  .card.crit::before {
    content: '';
    position: absolute;
    inset: 0 auto 0 0;
    width: 3px;
    background: var(--sev);
  }
  .card:hover {
    border-color: var(--line-strong);
  }
  .card:active {
    transform: scale(0.995);
  }
  .card:focus-visible {
    border-color: color-mix(in srgb, var(--accent) 70%, transparent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .card.missing {
    opacity: 0.6;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 11px;
    min-width: 0;
  }
  .title {
    flex: 1;
    min-width: 0;
  }
  .alias {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: var(--fs-15);
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .tag {
    width: 8px;
    height: 8px;
    flex: none;
    border-radius: 50%;
  }
  .plan {
    flex: none;
    padding: 1px 6px;
    border-radius: 5px;
    background: var(--surface-2);
    border: 1px solid var(--line);
    font-size: var(--fs-11);
    font-weight: 600;
    color: var(--text-2);
    letter-spacing: 0;
  }
  .fav {
    color: #ffb800;
    display: inline-flex;
    flex: none;
  }
  .who {
    margin-top: 2px;
    font-size: var(--fs-12);
    color: var(--text-2);
  }
  .who.email {
    color: var(--text);
    opacity: 0.86;
  }
  .state {
    display: flex;
    gap: 4px;
    flex: none;
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 20px;
    padding: 0 8px;
    border-radius: var(--r-pill);
    font-size: var(--fs-11);
    font-weight: 600;
    white-space: nowrap;
  }
  .pill i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
  }
  .pill.run {
    color: color-mix(in srgb, var(--run) 85%, var(--text));
    background: color-mix(in srgb, var(--run) 14%, transparent);
  }
  .pill.warn {
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 15%, transparent);
  }
  .pill.crit {
    color: var(--crit);
    background: color-mix(in srgb, var(--crit) 15%, transparent);
  }
  .more {
    opacity: 0;
    transition: opacity var(--dur) var(--ease);
  }
  .card:hover .more,
  .card:focus-within .more {
    opacity: 1;
  }
  .usage {
    flex: 1;
    min-height: 0;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: auto;
  }
  .meta {
    flex: 1;
    font-size: var(--fs-12);
    color: var(--text-3);
  }
  /* One action button, same size and place; only its meaning changes. */
  .act {
    min-width: 68px;
  }
  .act.switch {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    color: color-mix(in srgb, var(--accent) 55%, var(--text));
  }
  .act.switch:hover {
    background: color-mix(in srgb, var(--accent) 32%, transparent);
  }
  /* list layout */
  .card.list {
    flex-direction: row;
    align-items: center;
    padding: 10px 14px;
    gap: 16px;
  }
  .card.list .head {
    flex: 0 1 46%;
  }
  .card.list .usage {
    flex: 1;
    min-width: 0;
  }
</style>
