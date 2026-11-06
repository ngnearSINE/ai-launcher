<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { api } from '../lib/api'
  import { t } from '../lib/i18n.svelte'
  import { dialogs, run, ui } from '../lib/state.svelte'
  import Modal from './Modal.svelte'
  import Segmented from './Segmented.svelte'

  let tab = $state<'web' | 'program'>('web')
  const browsers = $derived((ui.snap?.installations ?? []).filter((i) => i.adapter === 'chromium' && i.present))
  let browser = $state('')
  $effect(() => {
    if (!browser && browsers.length) browser = browsers[0].id
  })
  let webName = $state('')
  let webUrl = $state('')
  let path = $state('')
  let progName = $state('')

  function close() {
    dialogs.addApp = false
  }
  async function addPreset(id: string) {
    if (await run(api.addWebapp({ preset: id, browser }), t('추가했어요'))) close()
  }
  async function addWeb() {
    if (await run(api.addWebapp({ name: webName, url: webUrl, browser }), t('추가했어요'))) close()
  }
  async function browse() {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: ui.snap?.os === 'windows' ? [{ name: 'Programs', extensions: ['exe'] }] : [],
    })
    if (typeof picked === 'string') path = picked
  }
  async function addProgram() {
    if (await run(api.addInstallation(path, progName || undefined), t('추가했어요'))) close()
  }
</script>

<Modal title={t('앱 추가')} width={540} onclose={close}>
  <div class="tabs">
    <Segmented value={tab} onchange={(v) => (tab = v)} options={[{ value: 'web', label: t('웹앱') }, { value: 'program', label: t('프로그램 경로') }]} />
  </div>

  {#if tab === 'web'}
    {#if !browsers.length}
      <p class="muted">{t('Chrome·Edge 같은 Chromium 브라우저가 있어야 웹앱을 따로 띄울 수 있어요.')}</p>
    {:else}
      <p class="muted small">{t('웹앱마다 독립된 브라우저 데이터 폴더를 써서 계정이 섞이지 않고, 작업 표시줄에도 따로 보입니다.')}</p>
      <div class="presets">
        {#each ui.snap?.presets ?? [] as p (p.id)}
          <button class="preset" onclick={() => addPreset(p.id)}>
            <span class="dot" style:background={p.color}></span>
            <span class="nm">{p.name}</span>
            <span class="url faint truncate">{p.url.replace('https://', '')}</span>
          </button>
        {/each}
      </div>
      <div class="custom">
        <input class="input" placeholder={t('이름')} bind:value={webName} />
        <input class="input" placeholder="https://" bind:value={webUrl} />
        <button class="btn" disabled={!webName || !webUrl} onclick={addWeb}>{t('추가')}</button>
      </div>
      {#if browsers.length > 1}
        <label class="br">
          <span class="faint small">{t('열 브라우저')}</span>
          <select class="select" bind:value={browser}>
            {#each browsers as b (b.id)}<option value={b.id}>{b.name}</option>{/each}
          </select>
        </label>
      {/if}
    {/if}
  {:else}
    <p class="muted small">{t('Electron 앱(Claude·Orca·Cursor 등)과 Chromium 브라우저는 자동으로 계정 분리가 됩니다. 그 밖의 프로그램은 실행만 관리해요.')}</p>
    <div class="custom">
      <input class="input mono" placeholder={ui.snap?.os === 'windows' ? 'C:\\…\\App.exe' : '/usr/bin/app'} bind:value={path} />
      <button class="btn" onclick={browse}>{t('찾아보기')}</button>
    </div>
    <input class="input" placeholder={t('표시 이름 (선택)')} bind:value={progName} />
  {/if}

  {#snippet footer()}
    <button class="btn" onclick={close}>{t('닫기')}</button>
    {#if tab === 'program'}<button class="btn primary" disabled={!path} onclick={addProgram}>{t('추가')}</button>{/if}
  {/snippet}
</Modal>

<style>
  .tabs {
    margin-bottom: 14px;
  }
  .small {
    font-size: var(--fs-12);
    line-height: 1.55;
    margin: 0 0 12px;
  }
  .presets {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
    margin-bottom: 14px;
  }
  .preset {
    display: grid;
    grid-template-columns: 10px auto 1fr;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 12px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--surface-2);
    text-align: left;
    min-width: 0;
  }
  .preset:hover {
    background: var(--surface-hover);
  }
  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
  .nm {
    font-weight: 500;
  }
  .url {
    font-size: var(--fs-11);
  }
  .custom {
    display: grid;
    grid-template-columns: 1fr 1.6fr auto;
    gap: 8px;
    margin-bottom: 10px;
  }
  .custom:has(.mono) {
    grid-template-columns: 1fr auto;
  }
  .mono {
    font-family: var(--mono);
    font-size: var(--fs-12);
  }
  .br {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .br .select {
    width: auto;
  }
</style>
