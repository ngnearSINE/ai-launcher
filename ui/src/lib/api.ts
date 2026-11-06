import { invoke } from '@tauri-apps/api/core'
import type { Instance, Installation, Settings, Snapshot } from './types'

type Patch<T> = Partial<T> & Record<string, unknown>

// Thin, typed wrappers over the Rust commands (app/src/commands.rs).
export const api = {
  state: () => invoke<Snapshot>('get_state'),
  launch: (id: string) => invoke<void>('launch', { id }),
  focus: (id: string) => invoke<boolean>('focus', { id }),
  closeWindows: (id: string) => invoke<boolean>('close_windows', { id }),
  quit: (id: string) => invoke<number>('quit', { id }),
  restart: (id: string) => invoke<void>('restart', { id }),
  createInstance: (a: { installationId: string; alias: string; color?: string; groupId?: string | null; useDefault?: boolean; launchNow?: boolean }) =>
    invoke<string>('create_instance', a),
  updateInstance: (id: string, patch: Patch<Instance>) => invoke<void>('update_instance', { id, patch }),
  duplicateInstance: (id: string) => invoke<string>('duplicate_instance', { id }),
  deleteInstance: (id: string, deleteData: boolean) => invoke<string[]>('delete_instance', { id, deleteData }),
  reorder: (ids: string[]) => invoke<void>('reorder_instances', { ids }),
  createGroup: (name: string) => invoke<string>('create_group', { name }),
  renameGroup: (id: string, name: string) => invoke<void>('rename_group', { id, name }),
  deleteGroup: (id: string) => invoke<void>('delete_group', { id }),
  addInstallation: (path: string, name?: string) => invoke<string>('add_installation', { path, name }),
  addWebapp: (a: { preset?: string; name?: string; url?: string; browser?: string }) => invoke<string>('add_webapp', a),
  updateInstallation: (id: string, patch: Patch<Installation>) => invoke<void>('update_installation', { id, patch }),
  removeInstallation: (id: string, withInstances: boolean) => invoke<void>('remove_installation', { id, withInstances }),
  rescan: () => invoke<void>('rescan'),
  updateSettings: (patch: Partial<Settings>) => invoke<void>('update_settings', { patch }),
  resetSettings: () => invoke<void>('reset_settings'),
  refreshUsage: (id?: string) => invoke<void>('refresh_usage', { id }),
  openProfile: (id: string) => invoke<void>('open_profile', { id }),
  openConfigDir: () => invoke<void>('open_config_dir'),
  createShortcut: (id: string, place: 'desktop' | 'startMenu') => invoke<string>('create_shortcut', { id, place }),
  profileSize: (id: string) => invoke<number>('profile_size', { id }),
  exportCatalog: (path: string) => invoke<void>('export_catalog', { path }),
  importCatalog: (path: string) => invoke<void>('import_catalog', { path }),
  showWindow: (label: 'main' | 'quick') => invoke<void>('show_window', { label }),
  hideWindow: (label: string) => invoke<void>('hide_window', { label }),
  setFloat: (on: boolean) => invoke<void>('set_float', { on }),
  launchBestClaude: () => invoke<string>('launch_best_claude'),
  quitApp: () => invoke<void>('quit_app'),
  appIcons: () => invoke<Record<string, string>>('app_icons'),
  switchView: (to: 'float' | 'main') => invoke<void>('switch_view', { to }),
  addInstalledWebApp:(appId: string) => invoke<string>('add_installed_web_app', { appId }),
  checkUpdate: () => invoke<void>('check_update'),
  installUpdate: () => invoke<boolean>('install_update'),
}
