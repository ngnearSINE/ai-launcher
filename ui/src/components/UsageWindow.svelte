<script lang="ts">
  // One usage window (5-hour, weekly, per-model, credits): label and value on top, a 6px bar
  // with the even-pace tick, and under it what to act on — the countdown to the reset, or when
  // it runs out if that comes first. In a table (`col` set) the column header already names the
  // window and says left/used, so a cell shows only the number — and its own label only when it
  // is a different kind of window than the column (Cursor's billing month in the weekly column).
  import { money } from '../lib/format'
  import { t } from '../lib/i18n.svelte'
  import { settings, ui } from '../lib/state.svelte'
  import type { Quota, QuotaKind } from '../lib/types'
  import { levelOf, win, windowNote } from '../lib/usage'

  let { q, quiet = false, col }: { q: Quota; quiet?: boolean; col?: QuotaKind } = $props()

  const s = $derived(settings())
  const w = $derived(win(q, s, ui.now))
  const level = $derived(levelOf(w, s))
  const label = $derived(
    q.kind === 'fiveHour' ? t('5시간') : q.kind === 'weekly' ? t('주간') : q.kind === 'credits' ? t('크레딧') : q.kind === 'monthly' && q.label === 'plan' ? t('이번 달') : q.label,
  )
  const inTable = $derived(col !== undefined)
  const showLabel = $derived(!inTable || q.kind !== col)
  const value = $derived(inTable ? `${Math.round(w.shown)}%` : `${Math.round(w.shown)}% ${s.usageValue === 'used' ? t('사용') : t('남음')}`)
  const note = $derived(
    q.kind === 'credits' && q.limitAmount ? `${money(q.usedAmount ?? 0, q.currency)} / ${money(q.limitAmount, q.currency)}` : s.usageShowReset || w.paceWarn ? windowNote(w, ui.now, inTable) : '',
  )
  // A table cell keeps the note on one line; the full sentence is the tooltip.
  const fullNote = $derived(inTable && w.paceWarn ? windowNote(w, ui.now) : undefined)
</script>

<div class="win {level}" class:quiet class:calm={s.usageBarTone !== 'accent'}>
  <div class="top">
    <span class="lab" class:colhide={!showLabel}>{label}</span>
    <span class="val num">{value}</span>
  </div>
  <span class="bar" role="meter" aria-label={label} aria-valuenow={Math.round(w.shown)} aria-valuemin="0" aria-valuemax="100">
    <span class="fill" style:width="{Math.min(100, w.shown)}%"></span>
    {#if w.paceAt !== null && q.kind !== 'credits'}<span class="pace" style:left="{w.paceAt}%" title={t('흰 눈금: 고르게 썼을 때의 위치')}></span>{/if}
  </span>
  {#if note}<div class="note num" class:hot={w.paceWarn} class:one={inTable} title={fullNote}>{note}</div>{/if}
</div>

<style>
  .win {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
    font-size: var(--fs-13);
  }
  .lab {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-2);
  }
  /* Named by the table's column header; the table shows it again when it stacks to one column. */
  .lab.colhide {
    display: none;
  }
  .val {
    flex: none;
    margin-left: auto;
    font-weight: 600;
  }
  .crit .val {
    color: var(--st-crit);
  }
  .warn .val {
    color: var(--st-warn);
  }
  .bar {
    position: relative;
    height: 6px;
    border-radius: 3px;
    background: var(--track);
  }
  .fill {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
    transition: width 500ms var(--ease);
  }
  /* Calm: a normal window is neutral so the one running low is what the eye finds. */
  .calm .fill {
    background: var(--fg-3);
  }
  .quiet .fill {
    background: var(--s4);
  }
  .warn .fill {
    background: var(--dot-warn);
  }
  .crit .fill {
    background: var(--dot-crit);
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
  .note {
    font-size: var(--fs-12);
    color: var(--text-3);
  }
  .note.one {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .note.hot {
    color: var(--st-warn);
  }
</style>
