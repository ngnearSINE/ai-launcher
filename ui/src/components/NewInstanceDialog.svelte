<script lang="ts">
  import { api } from '../lib/api'
  import { t } from '../lib/i18n.svelte'
  import { dialogs, run, ui } from '../lib/state.svelte'
  import Modal from './Modal.svelte'
  import Icon from './Icon.svelte'
  import { ColorPick, Switch } from './controls'

  const PALETTE = ['#0A84FF', '#FF9F0A', '#30D158', '#BF5AF2', '#FF375F', '#64D2FF', '#FFD60A', '#AC8E68', '#5E5CE6', '#FF6B3D']

  const installs = $derived((ui.snap?.installations ?? []).filter((i) => i.present && !i.hidden))
  const initial = typeof dialogs.newInstance === 'string' ? dialogs.newInstance : undefined
  let installationId = $state(initial ?? '')
  $effect(() => {
    if (!installationId && installs.length) installationId = installs[0].id
  })
  const install = $derived(installs.find((i) => i.id === installationId))
  const count = $derived((ui.snap?.instances ?? []).filter((i) => i.installationId === installationId).length)
  const defaultFree = $derived(!(ui.snap?.instances ?? []).some((i) => i.installationId === installationId && i.profile.kind === 'default'))

  let alias = $state('')
  let color = $state(PALETTE.find((c) => !(ui.snap?.instances ?? []).some((i) => i.color.toLowerCase() === c.toLowerCase())) ?? PALETTE[0])
  let groupId = $state<string>('')
  let useDefault = $state(false)
  let launchNow = $state(true)
  let busy = $state(false)
  const placeholder = $derived(install ? (count ? `${install.name} ${count + 1}` : install.name) : '')

  function close() {
    dialogs.newInstance = false
  }
  async function create() {
    if (!install) return
    busy = true
    const id = await run(
      api.createInstance({
        installationId,
        alias: alias.trim() || placeholder,
        color,
        groupId: groupId || null,
        useDefault: useDefault && defaultFree,
        launchNow,
      }),
    )
    busy = false
    if (id) close()
  }
</script>

<Modal title={t('새 인스턴스')} width={520} onclose={close}>
  <div class="field">
    <span class="lb">{t('앱')}</span>
    <div class="apps">
      {#each installs as i (i.id)}
        <button class="app" class:on={i.id === installationId} onclick={() => (installationId = i.id)}>
          {#if ui.icons[i.id]}<img src={ui.icons[i.id]} alt="" />{:else}<span class="tile" style:background={i.brand.color}>{i.brand.letter}</span>{/if}
          <span class="truncate">{i.name}</span>
        </button>
      {/each}
      <button class="app add" onclick={() => (dialogs.addApp = true)}><span class="tile plus"><Icon name="plus" size={16} /></span><span>{t('앱 추가')}</span></button>
    </div>
    {#if install?.isolation === 'none'}
      <p class="note warn"><Icon name="alert" size={13} /> {t('이 앱은 데이터 폴더를 나눌 방법을 모릅니다. 모든 인스턴스가 같은 로그인을 씁니다. 앱 설정에서 --user-data-dir 지원을 켤 수 있어요.')}</p>
    {/if}
  </div>

  <label class="field">
    <span class="lb">{t('별명')}</span>
    <!-- svelte-ignore a11y_autofocus -->
    <input class="input" bind:value={alias} {placeholder} autofocus onkeydown={(e) => e.key === 'Enter' && create()} />
  </label>

  <div class="field">
    <span class="lb">{t('색')}</span>
    <ColorPick value={color} onchange={(c) => (color = c)} />
  </div>

  {#if ui.snap?.groups.length}
    <label class="field">
      <span class="lb">{t('그룹')}</span>
      <select class="select" bind:value={groupId}>
        <option value="">{t('없음')}</option>
        {#each ui.snap.groups as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
      </select>
    </label>
  {/if}

  {#if install?.supportsDefaultProfile && defaultFree && install.isolation !== 'none'}
    <div class="opt">
      <div>
        <div>{t('앱 기본 프로필 사용')}</div>
        <div class="faint small">{t('평소 쓰던 로그인 그대로. 끄면 새 폴더를 만들어 따로 로그인해요.')}</div>
      </div>
      <Switch checked={useDefault} onchange={(v) => (useDefault = v)} />
    </div>
  {/if}
  <div class="opt">
    <div>{t('만들고 바로 실행')}</div>
    <Switch checked={launchNow} onchange={(v) => (launchNow = v)} />
  </div>

  {#snippet footer()}
    <button class="btn" onclick={close}>{t('취소')}</button>
    <button class="btn primary" disabled={!install || busy} onclick={create}>{t('만들기')}</button>
  {/snippet}
</Modal>

<style>
  .field {
    display: grid;
    gap: 8px;
    margin-bottom: 16px;
  }
  .lb {
    font-size: var(--fs-12);
    font-weight: 600;
    color: var(--text-2);
  }
  .apps {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 8px;
  }
  .app {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 10px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--surface-2);
    font-size: var(--fs-13);
    text-align: left;
    min-width: 0;
  }
  .app:hover {
    background: var(--surface-hover);
  }
  .app.on {
    border-color: var(--accent);
    background: var(--accent-softer);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .app img,
  .tile {
    width: 24px;
    height: 24px;
    flex: none;
  }
  .tile {
    display: grid;
    place-items: center;
    border-radius: 6px;
    color: white;
    font-weight: 700;
    font-size: 12px;
  }
  .tile.plus {
    background: var(--well);
    color: var(--text-2);
  }
  .add {
    color: var(--text-2);
    border-style: dashed;
  }
  .opt {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0;
    border-top: 1px solid var(--line);
  }
  .small {
    font-size: var(--fs-12);
  }
  .note {
    display: flex;
    gap: 6px;
    font-size: var(--fs-12);
    margin: 4px 0 0;
    line-height: 1.5;
  }
  .note.warn {
    color: var(--warn);
  }
</style>
