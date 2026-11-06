<script lang="ts">
  import { api } from '../lib/api'
  import { instanceMenu, launch, edit } from '../lib/actions'
  import { t } from '../lib/i18n.svelte'
  import { baseName } from '../lib/format'
  import { installationOf, run, ui, visibleOrgs } from '../lib/state.svelte'
  import type { Instance } from '../lib/types'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import UsageMeters from './UsageMeters.svelte'

  let {
    inst,
    list = false,
    index = 0,
    dragging = false,
    ondragstart,
    ondrop,
  }: { inst: Instance; list?: boolean; index?: number; dragging?: boolean; ondragstart?: (e: DragEvent) => void; ondrop?: (e: DragEvent) => void } = $props()

  const install = $derived(installationOf(inst))
  const email = $derived(inst.usage?.report?.email ?? '')
  const plan = $derived(visibleOrgs(inst)[0]?.plan ?? '')
  const missing = $derived(!install?.present)
  const webHost = $derived.by(() => {
    if (install?.adapter !== 'webapp') return ''
    try {
      return new URL(inst.launch.url || String(install.options.url ?? '')).host.replace(/^www\./, '')
    } catch {
      return ''
    }
  })
  const sub = $derived(
    [install?.name !== inst.alias ? install?.name : '', plan].filter(Boolean).join(' · ') ||
      webHost ||
      (inst.profile.kind === 'default' ? t('기본 프로필') : baseName(inst.profileDir)),
  )
  const browserName = $derived.by(() => {
    if (install?.adapter !== 'webapp') return ''
    const b = ui.snap?.installations.find((i) => i.id === install.options.browserInstall)
    return b?.name ?? ''
  })
  let over = $state(false)

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
  class="card"
  class:list
  class:running={inst.running}
  class:missing
  class:dragging
  class:over
  role="button"
  tabindex="0"
  draggable={ui.snap?.settings.sort === 'manual'}
  data-index={index}
  onclick={primary}
  onkeydown={onkey}
  oncontextmenu={(e) => instanceMenu(e, inst)}
  {ondragstart}
  ondragover={(e) => {
    e.preventDefault()
    over = true
  }}
  ondragleave={() => (over = false)}
  ondrop={(e) => {
    over = false
    ondrop?.(e)
  }}
>
  <div class="head">
    <Avatar {inst} size={list ? 32 : 40} />
    <div class="title">
      <div class="alias truncate">
        {inst.alias}
        {#if inst.favorite}<span class="fav"><Icon name="star" size={12} /></span>{/if}
      </div>
      <div class="sub truncate muted">{sub}</div>
    </div>
    {#if inst.running}
      <span class="state run"><i></i>{t('실행 중')}</span>
    {:else if missing}
      <span class="state warn">{t('앱 없음')}</span>
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
      <span class="meta faint truncate" title={email || inst.profileDir || ''}>
        {#if email}<Icon name="user" size={12} /> {email}{:else if inst.note}{inst.note}{:else if browserName}<Icon name="globe" size={12} /> {t('웹앱')} · {browserName}{:else}{inst.profile.kind === 'default' ? t('앱 기본 폴더') : baseName(inst.profileDir)}{/if}
      </span>
      <button
        class="btn sm"
        class:primary={!inst.running}
        disabled={missing}
        onclick={(e) => {
          e.stopPropagation()
          primary()
        }}
      >
        {#if inst.running}<Icon name="focus" size={14} /> {t('열기')}{:else}<Icon name="play" size={12} /> {t('실행')}{/if}
      </button>
    </div>
  {/if}
</div>

<style>
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: var(--sp-4);
    border-radius: var(--r-card);
    background: var(--surface);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-card);
    outline: none;
    transition: transform var(--dur) var(--ease), box-shadow var(--dur) var(--ease), border-color var(--dur) var(--ease);
  }
  .card:hover {
    border-color: var(--line-strong);
    box-shadow: var(--shadow-card), 0 6px 22px rgba(0, 0, 0, 0.06);
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
  .card.dragging {
    opacity: 0.4;
  }
  .card.over {
    border-color: var(--accent);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .title {
    flex: 1;
    min-width: 0;
  }
  .alias {
    font-size: var(--fs-15);
    font-weight: 600;
    letter-spacing: -0.01em;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .fav {
    color: #ffb800;
    display: inline-flex;
  }
  .sub {
    font-size: var(--fs-12);
    margin-top: 1px;
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-11);
    font-weight: 600;
    padding: 2px 8px;
    border-radius: var(--r-pill);
    white-space: nowrap;
  }
  .state.run {
    color: color-mix(in srgb, var(--run) 80%, var(--text));
    background: color-mix(in srgb, var(--run) 14%, transparent);
  }
  .state.run i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--run);
  }
  .state.warn {
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 14%, transparent);
  }
  .more {
    opacity: 0;
    transition: opacity var(--dur) var(--ease);
  }
  .card:hover .more,
  .card:focus-within .more {
    opacity: 1;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: auto;
  }
  .meta {
    font-size: var(--fs-12);
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  /* list layout */
  .card.list {
    flex-direction: row;
    align-items: center;
    padding: 10px 14px;
    gap: 16px;
  }
  .card.list .head {
    flex: 0 1 44%;
  }
  .card.list .usage {
    flex: 1;
    min-width: 0;
  }
</style>
