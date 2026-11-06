<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog'
  import { api } from '../lib/api'
  import { t } from '../lib/i18n.svelte'
  import { confirmBox, dialogs, run, settings, ui } from '../lib/state.svelte'
  import type { Settings as S } from '../lib/types'
  import Icon from './Icon.svelte'
  import { ColorPick, HotkeyInput, Row, Segmented, Switch } from './controls'

  const s = $derived(settings())
  function set<K extends keyof S>(key: K, value: S[K]) {
    run(api.updateSettings({ [key]: value } as Partial<S>))
  }

  const tabs = $derived([
    { id: 'general', icon: 'sliders', label: t('일반') },
    { id: 'appearance', icon: 'palette', label: t('모양') },
    { id: 'launcher', icon: 'float', label: t('런처') },
    { id: 'taskbar', icon: 'taskbar', label: t('작업 표시줄') },
    { id: 'usage', icon: 'chart', label: t('사용량') },
    { id: 'apps', icon: 'layers', label: t('앱 관리') },
    { id: 'data', icon: 'database', label: t('데이터') },
    { id: 'about', icon: 'info', label: t('정보') },
  ])
  const installs = $derived(ui.snap?.installations ?? [])
  const countOf = (id: string) => (ui.snap?.instances ?? []).filter((i) => i.installationId === id).length

  async function removeApp(id: string, name: string) {
    const n = countOf(id)
    const r = await confirmBox({
      title: t('{name} 빼기', { name }),
      message: n ? t('이 앱의 인스턴스 {n}개도 목록에서 빠집니다. 프로필 폴더는 그대로 남아요.', { n }) : t('런처 목록에서 뺍니다. 다시 검색하면 돌아와요.'),
      confirm: t('빼기'),
      danger: true,
    })
    if (r.ok) run(api.removeInstallation(id, true))
  }
  async function exportCat() {
    const p = await save({ defaultPath: 'ai-launcher-backup.json', filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (p) run(api.exportCatalog(p), t('내보냈어요'))
  }
  async function importCat() {
    const p = await open({ multiple: false, filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (typeof p === 'string') run(api.importCatalog(p), t('가져왔어요'))
  }
  async function reset() {
    const r = await confirmBox({ title: t('설정 초기화'), message: t('모든 설정을 기본값으로 되돌립니다. 인스턴스는 그대로예요.'), confirm: t('초기화'), danger: true })
    if (r.ok) run(api.resetSettings(), t('초기화했어요'))
  }
</script>

<div class="settings">
  <nav>
    <button class="back" onclick={() => (ui.view = 'home')}><Icon name="chevron" size={14} /> {t('돌아가기')}</button>
    {#each tabs as tb (tb.id)}
      <button class="tab" class:on={ui.settingsTab === tb.id} onclick={() => (ui.settingsTab = tb.id)}>
        <Icon name={tb.icon} size={15} />
        {tb.label}
      </button>
    {/each}
  </nav>

  <section>
    {#if ui.settingsTab === 'general'}
      <h1>{t('일반')}</h1>
      <Row label={t('언어')}>
        <Segmented value={s.language} onchange={(v) => set('language', v)} options={[{ value: 'system', label: t('시스템') }, { value: 'ko', label: '한국어' }, { value: 'en', label: 'English' }]} />
      </Row>
      <Row label={t('컴퓨터를 켤 때 런처 시작')} desc={t('트레이에서 조용히 시작해요')}>
        <Switch checked={s.startWithOs} onchange={(v) => set('startWithOs', v)} />
      </Row>
      <Row label={t('창을 닫으면 트레이로')} desc={t('끄면 창을 닫을 때 런처도 끝나요(실행 중인 앱은 그대로)')}>
        <Switch checked={s.closeToTray} onchange={(v) => set('closeToTray', v)} />
      </Row>
      <Row label={t('이미 실행 중이면 앞으로 가져오기')} desc={t('끄면 실행을 누를 때마다 앱에 다시 실행 요청을 보내요')}>
        <Switch checked={s.focusIfRunning} onchange={(v) => set('focusIfRunning', v)} />
      </Row>
      <Row label={t('실행 후 런처 창 숨기기')}>
        <Switch checked={s.hideAfterLaunch} onchange={(v) => set('hideAfterLaunch', v)} />
      </Row>
      <Row label={t('강제 종료 전에 묻기')}>
        <Switch checked={s.confirmQuitInstance} onchange={(v) => set('confirmQuitInstance', v)} />
      </Row>
      <Row label={t('정렬')}>
        <select class="select w" value={s.sort} onchange={(e) => set('sort', (e.target as HTMLSelectElement).value as S['sort'])}>
          <option value="manual">{t('직접 (끌어서 순서 바꾸기)')}</option>
          <option value="name">{t('이름순')}</option>
          <option value="recent">{t('최근 실행순')}</option>
          <option value="headroom">{t('사용량 여유순')}</option>
        </select>
      </Row>
    {:else if ui.settingsTab === 'appearance'}
      <h1>{t('모양')}</h1>
      <Row label={t('테마')}>
        <Segmented value={s.theme} onchange={(v) => set('theme', v)} options={[{ value: 'system', label: t('시스템') }, { value: 'light', label: t('밝게') }, { value: 'dark', label: t('어둡게') }]} />
      </Row>
      <Row label={t('강조색')} stack>
        <ColorPick value={s.accent} onchange={(c) => set('accent', c)} />
      </Row>
      <Row label={t('유리 효과')} desc={t('창 배경에 Windows Mica·아크릴 효과를 씁니다')}>
        <Switch checked={s.glass} onchange={(v) => set('glass', v)} />
      </Row>
      <Row label={t('유리 효과 강도')} desc={`${s.glassStrength}`}>
        <input type="range" min="0" max="100" value={s.glassStrength} disabled={!s.glass} onchange={(e) => set('glassStrength', +(e.target as HTMLInputElement).value)} />
      </Row>
      <Row label={t('카드 밀도')}>
        <Segmented value={s.density} onchange={(v) => set('density', v)} options={[{ value: 'comfortable', label: t('여유') }, { value: 'compact', label: t('촘촘') }]} />
      </Row>
      <Row label={t('카드 사용량 표시')} desc={t('촘촘하게는 5시간·주간만')}>
        <Segmented value={s.cardUsageStyle} onchange={(v) => set('cardUsageStyle', v)} options={[{ value: 'bars', label: t('전부') }, { value: 'compact', label: t('촘촘하게') }]} />
      </Row>
      <Row label={t('움직임 줄이기')}>
        <Switch checked={s.reduceMotion} onchange={(v) => set('reduceMotion', v)} />
      </Row>
    {:else if ui.settingsTab === 'launcher'}
      <h1>{t('런처')}</h1>
      <h2>{t('빠른 실행')}</h2>
      <Row label={t('빠른 실행 단축키')} desc={t('어디서든 눌러 검색하고 Enter로 실행')}>
        <HotkeyInput value={s.quickHotkey} onchange={(v) => set('quickHotkey', v)} />
        <Switch checked={s.quickHotkeyEnabled} onchange={(v) => set('quickHotkeyEnabled', v)} />
      </Row>
      <Row label={t('가장 여유 있는 Claude 실행')} desc={t('5시간·주간 사용량이 가장 낮은 Claude 인스턴스를 띄워요')}>
        <HotkeyInput value={s.bestClaudeHotkey} onchange={(v) => set('bestClaudeHotkey', v)} />
        <Switch checked={s.bestClaudeHotkeyEnabled} onchange={(v) => set('bestClaudeHotkeyEnabled', v)} />
      </Row>
      <Row label={t('트레이 아이콘 클릭')}>
        <Segmented value={s.trayLeftClick} onchange={(v) => set('trayLeftClick', v)} options={[{ value: 'popup', label: t('빠른 실행') }, { value: 'main', label: t('메인 창') }]} />
      </Row>
      <h2>{t('플로팅 런처')}</h2>
      <Row label={t('플로팅 런처 표시')} desc={t('화면 가장자리에 떠 있는 작은 아이콘 줄')}>
        <Switch checked={s.floatEnabled} onchange={(v) => set('floatEnabled', v)} />
      </Row>
      <Row label={t('방향')}>
        <Segmented value={s.floatOrientation} onchange={(v) => set('floatOrientation', v)} options={[{ value: 'vertical', label: t('세로') }, { value: 'horizontal', label: t('가로') }]} />
      </Row>
      <Row label={t('아이콘 크기')} desc={`${s.floatSize}px`}>
        <input type="range" min="28" max="72" value={s.floatSize} onchange={(e) => set('floatSize', +(e.target as HTMLInputElement).value)} />
      </Row>
      <Row label={t('불투명도')} desc={`${s.floatOpacity}%`}>
        <input type="range" min="30" max="100" value={s.floatOpacity} onchange={(e) => set('floatOpacity', +(e.target as HTMLInputElement).value)} />
      </Row>
      <Row label={t('항상 위에')}>
        <Switch checked={s.floatAlwaysOnTop} onchange={(v) => set('floatAlwaysOnTop', v)} />
      </Row>
      <Row label={t('사용량 고리 표시')} desc={t('아이콘 둘레에 5시간·주간 중 높은 쪽')}>
        <Switch checked={s.floatShowUsage} onchange={(v) => set('floatShowUsage', v)} />
      </Row>
      <Row label={t('위치 초기화')}>
        <button class="btn sm" onclick={() => set('floatPosition', null)}>{t('초기화')}</button>
      </Row>
    {:else if ui.settingsTab === 'taskbar'}
      <h1>{t('작업 표시줄')}</h1>
      <p class="lead">{t('여기 설정은 모든 인스턴스의 기본값입니다. 인스턴스마다 편집 → 고급에서 따로 바꿀 수 있어요.')}</p>
      {#if ui.snap?.os !== 'windows'}<p class="lead warn">{t('작업 표시줄 분리는 Windows 기능이에요. Linux에서는 앱 메뉴 항목으로 고정하세요.')}</p>{/if}
      <Row label={t('인스턴스마다 작업 표시줄 버튼 나누기')} desc={t('같은 앱의 다른 계정 창을 별도 버튼으로 나눠, 각각 작업 표시줄에 고정할 수 있어요. 끄면 앱 하나로 합쳐 보여요.')}>
        <Switch checked={s.taskbarSeparate} onchange={(v) => set('taskbarSeparate', v)} />
      </Row>
      <Row label={t('기본 프로필 인스턴스도 나누기')} desc={t('끄면 앱을 평소대로 연 기본 인스턴스는 원래 앱 버튼에 그대로 둡니다')}>
        <Switch checked={s.taskbarTagDefaultInstance} disabled={!s.taskbarSeparate} onchange={(v) => set('taskbarTagDefaultInstance', v)} />
      </Row>
      <Row label={t('색 배지 아이콘')} desc={t('나눈 버튼의 아이콘 구석에 인스턴스 색 점을 그려 구분해요')}>
        <Switch checked={s.taskbarBadge} onchange={(v) => set('taskbarBadge', v)} />
      </Row>
      <Row label={t('고정한 버튼은 런처를 거쳐 실행')} desc={t('고정 버튼을 누르면 런처가 그 인스턴스 폴더로 정확히 띄워요. 끄면 앱 기본 실행(기본 프로필)이 열려요.')}>
        <Switch checked={s.taskbarRelaunchThroughLauncher} onchange={(v) => set('taskbarRelaunchThroughLauncher', v)} />
      </Row>
      <Row label={t('실행 상태 확인 주기')} desc={t('{n}초마다. 짧을수록 반응이 빠르고 CPU를 조금 더 써요', { n: s.taskbarRescanSeconds })}>
        <input type="range" min="1" max="10" value={s.taskbarRescanSeconds} onchange={(e) => set('taskbarRescanSeconds', +(e.target as HTMLInputElement).value)} />
      </Row>
      <div class="tip">
        <Icon name="info" size={14} />
        <span>{t('고정하는 법: 인스턴스를 실행한 뒤 작업 표시줄에 생긴 버튼을 우클릭 → "작업 표시줄에 고정". 고정한 뒤에는 버튼 이름·아이콘을 바꿔도 Windows가 예전 것을 기억할 수 있어요.')}</span>
      </div>
    {:else if ui.settingsTab === 'usage'}
      <h1>{t('사용량')}</h1>
      <p class="lead">{t('사용량은 각 인스턴스가 쓰는 프로필 폴더의 로그인으로만 조회해요. 다른 인스턴스나 CLI의 로그인을 섞어 쓰지 않습니다. 로그인 정보는 이 PC 안에서만 풀고 저장하지 않아요.')}</p>
      <Row label={t('사용량 조회')}>
        <Switch checked={s.usageEnabled} onchange={(v) => set('usageEnabled', v)} />
      </Row>
      <Row label={t('새로고침 주기')} desc={t('{n}분 (서비스 요청 제한 때문에 2분보다 짧게는 묻지 않아요)', { n: s.usageRefreshMinutes })}>
        <input type="range" min="2" max="60" value={s.usageRefreshMinutes} onchange={(e) => set('usageRefreshMinutes', +(e.target as HTMLInputElement).value)} />
      </Row>
      <Row label={t('초기화까지 남은 시간 표시')}>
        <Switch checked={s.usageShowReset} onchange={(v) => set('usageShowReset', v)} />
      </Row>
      <Row label={t('주의 색 기준')} desc={`${s.usageWarnPercent}%`}>
        <input type="range" min="30" max="95" value={s.usageWarnPercent} onchange={(e) => set('usageWarnPercent', +(e.target as HTMLInputElement).value)} />
      </Row>
      <Row label={t('위험 색 기준')} desc={`${s.usageCritPercent}%`}>
        <input type="range" min="50" max="100" value={s.usageCritPercent} onchange={(e) => set('usageCritPercent', +(e.target as HTMLInputElement).value)} />
      </Row>
      <Row label={t('지금 모두 새로고침')}>
        <button class="btn sm" onclick={() => run(api.refreshUsage())}><Icon name="refresh" size={13} /> {t('새로고침')}</button>
      </Row>
    {:else if ui.settingsTab === 'apps'}
      <h1>{t('앱 관리')}</h1>
      <p class="lead">{t('기본 경로에 설치된 앱은 자동으로 찾아요. 다른 위치의 프로그램은 경로로 추가하세요.')}</p>
      <div class="bar">
        <button class="btn sm" onclick={() => run(api.rescan(), t('다시 찾았어요'))}><Icon name="refresh" size={13} /> {t('다시 찾기')}</button>
        <button class="btn sm primary" onclick={() => (dialogs.addApp = true)}><Icon name="plus" size={13} /> {t('앱 추가')}</button>
      </div>
      <div class="apps">
        {#each installs as i (i.id)}
          <div class="app" class:missing={!i.present}>
            {#if ui.icons[i.id]}<img src={ui.icons[i.id]} alt="" />{:else}<span class="tile" style:background={i.brand.color}>{i.brand.letter}</span>{/if}
            <div class="info">
              <div class="nm">{i.name} {#if !i.present}<span class="badge warn">{t('찾을 수 없음')}</span>{/if}</div>
              <div class="faint small truncate" title={i.exe}>
                {i.adapter === 'webapp' ? String(i.options.url ?? '') : i.exe}
              </div>
              <div class="faint small">
                {i.isolation === 'userDataDir' ? t('계정 분리 지원') : t('계정 분리 없음')} · {t('인스턴스 {n}개', { n: countOf(i.id) })}
              </div>
            </div>
            <div class="ops">
              {#if i.adapter === 'custom'}
                <label class="chk" title="--user-data-dir">
                  <input type="checkbox" checked={i.options.userDataDir === true} onchange={(e) => run(api.updateInstallation(i.id, { options: { userDataDir: (e.target as HTMLInputElement).checked } }))} />
                  --user-data-dir
                </label>
              {/if}
              <button class="icon-btn sm" title={i.hidden ? t('보이기') : t('숨기기')} onclick={() => run(api.updateInstallation(i.id, { hidden: !i.hidden }))}><Icon name={i.hidden ? 'eye-off' : 'eye'} size={15} /></button>
              <button class="icon-btn sm" title={t('빼기')} onclick={() => removeApp(i.id, i.name)}><Icon name="trash" size={15} /></button>
            </div>
          </div>
        {/each}
      </div>
    {:else if ui.settingsTab === 'data'}
      <h1>{t('데이터')}</h1>
      <Row label={t('삭제할 때 휴지통으로')} desc={t('끄면 프로필 폴더를 바로 지워요')}>
        <Switch checked={s.deleteMovesToTrash} onchange={(v) => set('deleteMovesToTrash', v)} />
      </Row>
      <Row label={t('인스턴스 삭제 시 폴더도 지우기를 기본으로')}>
        <Switch checked={s.deleteProfileByDefault} onchange={(v) => set('deleteProfileByDefault', v)} />
      </Row>
      <Row label={t('웹앱 프로필 저장 위치')} desc={s.profilesRoot || t('기본 위치')} stack>
        <div class="btns">
          <button
            class="btn sm"
            onclick={async () => {
              const p = await open({ directory: true })
              if (typeof p === 'string') set('profilesRoot', p)
            }}>{t('바꾸기')}</button
          >
          {#if s.profilesRoot}<button class="btn sm" onclick={() => set('profilesRoot', null)}>{t('기본으로')}</button>{/if}
        </div>
      </Row>
      <Row label={t('백업')} desc={t('인스턴스 목록과 설정(로그인 정보 제외)')}>
        <button class="btn sm" onclick={exportCat}>{t('내보내기')}</button>
        <button class="btn sm" onclick={importCat}>{t('가져오기')}</button>
      </Row>
      <Row label={t('설정 폴더')} desc={ui.snap?.configDir ?? ''}>
        <button class="btn sm" onclick={() => run(api.openConfigDir())}><Icon name="folder" size={13} /> {t('열기')}</button>
      </Row>
      <Row label={t('설정 초기화')}>
        <button class="btn sm danger" onclick={reset}>{t('초기화')}</button>
      </Row>
    {:else}
      <h1>{t('정보')}</h1>
      <div class="about">
        <img src="/icon.png" alt="" />
        <div>
          <div class="nm">AI Launcher</div>
          <div class="faint">{t('버전')} {ui.snap?.version}</div>
        </div>
      </div>
      <p class="lead">{t('여러 계정의 AI 데스크톱 앱을 나란히 띄우는 런처. 각 인스턴스는 자기 데이터 폴더를 쓰고, 사용량은 그 폴더의 로그인으로만 조회합니다.')}</p>
      <Row label={t('종료')} desc={t('런처를 끝내도 실행 중인 앱은 그대로예요')}>
        <button class="btn sm" onclick={() => api.quitApp()}>{t('런처 종료')}</button>
      </Row>
    {/if}
  </section>
</div>

<style>
  .settings {
    display: grid;
    grid-template-columns: 200px 1fr;
    height: 100%;
    min-height: 0;
  }
  nav {
    padding: 12px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-right: 1px solid var(--line);
  }
  .back {
    display: flex;
    align-items: center;
    gap: 4px;
    border: 0;
    background: none;
    color: var(--accent);
    height: 30px;
    padding: 0 8px;
    margin-bottom: 8px;
    font-size: var(--fs-13);
  }
  .back :global(svg) {
    transform: rotate(180deg);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--r-md);
    background: transparent;
    color: var(--text-2);
    font-size: var(--fs-13);
    text-align: left;
  }
  .tab:hover {
    background: var(--surface-hover);
    color: var(--text);
  }
  .tab.on {
    background: var(--accent-soft);
    color: var(--text);
    font-weight: 600;
  }
  .tab.on :global(svg) {
    color: var(--accent);
  }
  section {
    overflow: auto;
    padding: 20px 32px 40px;
    max-width: 760px;
  }
  h1 {
    font-size: var(--fs-22);
    font-weight: 700;
    letter-spacing: -0.02em;
    margin: 4px 0 12px;
  }
  h2 {
    font-size: var(--fs-12);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-3);
    margin: 22px 0 2px;
  }
  .lead {
    color: var(--text-2);
    font-size: var(--fs-13);
    line-height: 1.6;
    margin: 0 0 8px;
  }
  .lead.warn {
    color: var(--warn);
  }
  .w {
    width: 220px;
  }
  input[type='range'] {
    width: 180px;
    accent-color: var(--accent);
  }
  .tip {
    display: flex;
    gap: 8px;
    margin-top: 16px;
    padding: 12px;
    border-radius: var(--r-lg);
    background: var(--accent-softer);
    font-size: var(--fs-12);
    color: var(--text-2);
    line-height: 1.6;
  }
  .tip :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent);
  }
  .bar {
    display: flex;
    gap: 8px;
    margin: 4px 0 12px;
  }
  .apps {
    display: grid;
    gap: 8px;
  }
  .app {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--r-lg);
    background: var(--surface);
    border: 1px solid var(--line);
  }
  .app.missing {
    opacity: 0.6;
  }
  .app img,
  .tile {
    width: 32px;
    height: 32px;
    flex: none;
  }
  .tile {
    display: grid;
    place-items: center;
    border-radius: 8px;
    color: white;
    font-weight: 700;
  }
  .info {
    flex: 1;
    min-width: 0;
  }
  .nm {
    font-weight: 600;
  }
  .small {
    font-size: var(--fs-12);
  }
  .ops {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .chk {
    display: flex;
    gap: 5px;
    align-items: center;
    font-size: var(--fs-11);
    font-family: var(--mono);
    color: var(--text-2);
    margin-right: 6px;
  }
  .badge {
    font-size: var(--fs-11);
    padding: 1px 6px;
    border-radius: var(--r-pill);
    font-weight: 500;
  }
  .badge.warn {
    color: var(--warn);
    background: color-mix(in srgb, var(--warn) 14%, transparent);
  }
  .btns {
    display: flex;
    gap: 8px;
  }
  .about {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 14px;
  }
  .about img {
    width: 56px;
    height: 56px;
  }
  .about .nm {
    font-size: var(--fs-17);
  }
</style>
