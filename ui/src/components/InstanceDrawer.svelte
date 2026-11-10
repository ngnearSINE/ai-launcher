<script lang="ts">
  import { api } from '../lib/api'
  import { deleteInstance } from '../lib/actions'
  import { bytes, ago } from '../lib/format'
  import { t } from '../lib/i18n.svelte'
  import { installationOf, instanceById, run, ui } from '../lib/state.svelte'
  import type { Instance, InstanceAdvanced, LaunchOptions, Tristate } from '../lib/types'
  import Avatar from './Avatar.svelte'
  import Icon from './Icon.svelte'
  import { ColorPick, Row, Segmented, Switch } from './controls'

  const inst = $derived(instanceById(ui.drawer))
  const install = $derived(inst ? installationOf(inst) : undefined)
  let tab = $state<'general' | 'launch' | 'advanced'>('general')
  let size = $state<number | null>(null)
  let lastId = ''

  $effect(() => {
    if (inst && inst.id !== lastId) {
      lastId = inst.id
      size = null
      api.profileSize(inst.id).then((n) => (size = n)).catch(() => (size = null))
    }
  })

  function patch(p: Partial<Instance>) {
    if (inst) run(api.updateInstance(inst.id, p))
  }
  function patchLaunch(p: Partial<LaunchOptions>) {
    if (inst) patch({ launch: { ...inst.launch, ...p } })
  }
  function patchAdv(p: Partial<InstanceAdvanced>) {
    if (inst) patch({ advanced: { ...inst.advanced, ...p } })
  }
  const tri = $derived([
    { value: 'inherit' as Tristate, label: t('전체 설정') },
    { value: 'on' as Tristate, label: t('켜기') },
    { value: 'off' as Tristate, label: t('끄기') },
  ])
  function argsText(a: string[]) {
    return a.join('\n')
  }
  function envText(e: [string, string][]) {
    return e.map(([k, v]) => `${k}=${v}`).join('\n')
  }
  function parseEnv(s: string): [string, string][] {
    return s
      .split('\n')
      .map((l) => l.trim())
      .filter((l) => l.includes('='))
      .map((l) => [l.slice(0, l.indexOf('=')).trim(), l.slice(l.indexOf('=') + 1)] as [string, string])
  }
  const orgs = $derived(inst?.usage?.report?.orgs ?? [])
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && ui.drawer && !document.querySelector('.scrim') && (ui.drawer = null)} />

{#if inst}
  <div class="scrim-lite" role="presentation" onmousedown={() => (ui.drawer = null)}></div>
  <aside class="drawer" aria-label={inst.alias}>
    <header>
      <Avatar {inst} size={44} />
      <div class="ttl">
        <div class="name truncate">{inst.alias}</div>
        <div class="muted small truncate">{install?.name}{inst.running ? ` · ${t('실행 중')}` : ''}</div>
      </div>
      <button class="icon-btn" aria-label={t('닫기')} onclick={() => (ui.drawer = null)}><Icon name="x" size={18} /></button>
    </header>

    <div class="tabs">
      <Segmented
        value={tab}
        onchange={(v) => (tab = v)}
        options={[
          { value: 'general', label: t('일반') },
          { value: 'launch', label: t('실행') },
          { value: 'advanced', label: t('고급') },
        ]}
      />
    </div>

    <div class="body">
      {#if tab === 'general'}
        <Row label={t('별명')} stack>
          <input class="input" value={inst.alias} onchange={(e) => patch({ alias: (e.target as HTMLInputElement).value })} />
        </Row>
        <Row label={t('색')} desc={t('카드·플로팅 런처·작업 표시줄 배지에 쓰여요')} stack>
          <ColorPick value={inst.color} onchange={(c) => patch({ color: c })} />
        </Row>
        <Row label={t('글자 표식')} desc={t('아이콘이 없을 때 타일에 보일 1~2글자')}>
          <input class="input glyph" maxlength="2" value={inst.glyph ?? ''} onchange={(e) => patch({ glyph: (e.target as HTMLInputElement).value || null })} />
        </Row>
        <Row label={t('그룹')}>
          <select class="select" value={inst.groupId ?? ''} onchange={(e) => patch({ groupId: (e.target as HTMLSelectElement).value || null })}>
            <option value="">{t('없음')}</option>
            {#each ui.snap?.groups ?? [] as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
          </select>
        </Row>
        <Row label={t('즐겨찾기')} desc={t('빠른 실행 목록 맨 위에 둡니다')}>
          <Switch checked={inst.favorite} onchange={(v) => patch({ favorite: v })} />
        </Row>
        <Row label={t('메모')} stack>
          <textarea class="textarea plain" rows="2" value={inst.note} onchange={(e) => patch({ note: (e.target as HTMLTextAreaElement).value })}></textarea>
        </Row>
      {:else if tab === 'launch'}
        {#if install?.adapter === 'webapp'}
          <Row label={t('주소')} stack>
            <input class="input" placeholder={String(install.options.url ?? '')} value={inst.launch.url ?? ''} onchange={(e) => patchLaunch({ url: (e.target as HTMLInputElement).value || null })} />
          </Row>
        {/if}
        <Row label={t('추가 인자')} desc={t('한 줄에 하나씩. 런처가 붙이는 --user-data-dir 뒤에 붙어요')} stack>
          <textarea class="textarea" rows="3" value={argsText(inst.launch.args)} onchange={(e) => patchLaunch({ args: (e.target as HTMLTextAreaElement).value.split('\n').map((s) => s.trim()).filter(Boolean) })}></textarea>
        </Row>
        <Row label={t('환경 변수')} desc="KEY=VALUE" stack>
          <textarea class="textarea" rows="3" value={envText(inst.launch.env)} onchange={(e) => patchLaunch({ env: parseEnv((e.target as HTMLTextAreaElement).value) })}></textarea>
        </Row>
        <Row label={t('런처와 함께 자동 실행')} desc={t('컴퓨터를 켜서 런처가 시작될 때 이 인스턴스도 띄워요')}>
          <Switch checked={inst.launch.autostart} onchange={(v) => patchLaunch({ autostart: v })} />
        </Row>
        <div class="stats faint small">
          {t('마지막 실행')} {ago(inst.lastLaunchedAt, ui.now)} · {t('{n}회 실행', { n: inst.launchCount })}
        </div>
      {:else}
        <Row label={t('작업 표시줄에 따로 표시')} desc={t('이 인스턴스 창을 별도 버튼으로 나눠 따로 고정할 수 있게 해요')} stack>
          <Segmented size="sm" value={inst.advanced.taskbarSeparate} options={tri} onchange={(v) => patchAdv({ taskbarSeparate: v })} />
          {#if inst.taskbarNeedsRestart}
            <div class="note">
              <Icon name="info" size={13} />
              <span>{t('이 앱은 다시 시작해야 원래 버튼으로 합쳐져요')}</span>
              <button class="btn sm" onclick={() => run(api.restart(inst.id))}>{t('다시 시작')}</button>
            </div>
          {/if}
        </Row>
        <Row label={t('색 배지 아이콘')} desc={t('작업 표시줄·바로가기 아이콘 구석에 인스턴스 색 점')} stack>
          <Segmented size="sm" value={inst.advanced.taskbarBadge} options={tri} onchange={(v) => patchAdv({ taskbarBadge: v })} />
        </Row>
        <Row label={t('고정 아이콘은 런처로 실행')} desc={t('고정한 버튼을 누르면 런처가 이 인스턴스를 정확히 띄워요')} stack>
          <Segmented size="sm" value={inst.advanced.relaunchThroughLauncher} options={tri} onchange={(v) => patchAdv({ relaunchThroughLauncher: v })} />
        </Row>
        {#if install?.hasUsage}
          <Row label={t('사용량 조회')} desc={t('이 인스턴스 프로필의 로그인만 사용해요')} stack>
            <Segmented size="sm" value={inst.advanced.usageEnabled} options={tri} onchange={(v) => patchAdv({ usageEnabled: v })} />
          </Row>
          {#if orgs.length > 1}
            <Row label={t('표시할 조직')} desc={t('고르지 않으면 모두 보여요')} stack>
              <div class="orgs">
                {#each orgs as o (o.orgUuid)}
                  <label class="chk">
                    <input
                      type="checkbox"
                      checked={inst.advanced.usageOrgs.includes(o.orgUuid)}
                      onchange={(e) => {
                        const on = (e.target as HTMLInputElement).checked
                        const set = new Set(inst.advanced.usageOrgs)
                        if (on) set.add(o.orgUuid)
                        else set.delete(o.orgUuid)
                        patchAdv({ usageOrgs: [...set] })
                      }}
                    />
                    {o.orgName} <span class="faint">{o.plan}</span>
                  </label>
                {/each}
              </div>
            </Row>
          {/if}
        {/if}
        <Row label={t('빠른 실행·플로팅에서 숨기기')}>
          <Switch checked={inst.advanced.hiddenInQuick} onchange={(v) => patchAdv({ hiddenInQuick: v })} />
        </Row>
        <Row label={t('프로필 폴더')} desc={inst.profileDir ?? t('없음')} stack>
          <div class="btns">
            <button class="btn sm" onclick={() => run(api.openProfile(inst.id))}><Icon name="folder" size={13} /> {t('열기')}</button>
            <span class="faint small">{size === null ? '…' : bytes(size)}</span>
          </div>
        </Row>
        <Row label={t('바로가기')} stack>
          <div class="btns">
            <button class="btn sm" onclick={() => run(api.createShortcut(inst.id, 'desktop'), t('바탕화면에 만들었어요'))}>{t('바탕화면')}</button>
            <button class="btn sm" onclick={() => run(api.createShortcut(inst.id, 'startMenu'), t('추가했어요'))}>{ui.snap?.os === 'linux' ? t('앱 메뉴') : t('시작 메뉴')}</button>
          </div>
        </Row>
        <Row label={t('작업 표시줄 ID')} desc={inst.appId} stack>
          <span></span>
        </Row>
        <div class="danger-zone">
          <button class="btn" onclick={() => run(api.duplicateInstance(inst.id), t('복제했어요'))}><Icon name="copy" size={14} /> {t('복제')}</button>
          <button class="btn danger" onclick={() => deleteInstance(inst)}><Icon name="trash" size={14} /> {t('삭제…')}</button>
        </div>
      {/if}
    </div>
  </aside>
{/if}

<style>
  .scrim-lite {
    position: fixed;
    inset: 40px 0 0 0;
    z-index: 30;
  }
  .drawer {
    position: fixed;
    top: 48px;
    right: 8px;
    bottom: 8px;
    width: min(400px, calc(100vw - 16px));
    z-index: 31;
    display: flex;
    flex-direction: column;
    border-radius: var(--r-modal);
    background: color-mix(in srgb, var(--surface-solid) 96%, transparent);
    backdrop-filter: blur(30px) saturate(1.6);
    border: 1px solid var(--line);
    box-shadow: var(--shadow-pop);
    animation: slide 220ms var(--ease);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 18px 14px 10px 18px;
  }
  .ttl {
    flex: 1;
    min-width: 0;
  }
  .name {
    font-size: var(--fs-17);
    font-weight: 650;
  }
  .small {
    font-size: var(--fs-12);
  }
  .tabs {
    padding: 0 18px 4px;
  }
  .body {
    flex: 1;
    overflow: auto;
    padding: 4px 18px 18px;
  }
  .glyph {
    width: 64px;
    text-align: center;
  }
  .select {
    width: 160px;
  }
  .textarea.plain {
    font-family: var(--font);
    font-size: var(--fs-13);
  }
  .btns {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .orgs {
    display: grid;
    gap: 6px;
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--fs-13);
  }
  .note {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-12);
    color: var(--warn);
  }
  .note span {
    flex: 1;
  }
  .stats {
    padding-top: 12px;
  }
  .danger-zone {
    display: flex;
    gap: 8px;
    padding-top: 16px;
  }
  @keyframes slide {
    from {
      opacity: 0;
      transform: translateX(24px);
    }
  }
</style>
