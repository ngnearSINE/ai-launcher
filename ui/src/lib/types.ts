// Mirrors app/src/snapshot.rs (serde camelCase).

export type Tristate = 'inherit' | 'on' | 'off'

export interface Settings {
  language: 'system' | 'ko' | 'en'
  theme: 'system' | 'light' | 'dark' | 'claude' | 'midnight' | 'black'
  accent: string
  glass: boolean
  glassStrength: number
  backgroundEffect: 'tabbed' | 'mica' | 'acrylic' | 'none'
  reduceMotion: boolean
  density: 'comfortable' | 'compact'
  cardUsageStyle: 'bars' | 'compact'
  sort: 'manual' | 'name' | 'recent' | 'headroom'
  groupBy: 'none' | 'app' | 'group'
  sidebarGroup: 'status' | 'app' | 'group' | 'none'
  sidebarShowAccount: boolean
  sidebarShow5h: boolean
  sidebarShowWeekly: boolean
  keymap: Record<string, string>
  startWithOs: boolean
  startHidden: boolean
  focusIfRunning: boolean
  hideAfterLaunch: boolean
  confirmQuitInstance: boolean
  quickHotkey: string
  quickHotkeyEnabled: boolean
  bestClaudeHotkey: string
  bestClaudeHotkeyEnabled: boolean
  trayLeftClick: 'popup' | 'main'
  trayDoubleClick: 'main' | 'quick' | 'none'
  floatOrientation: 'vertical' | 'horizontal'
  floatAlwaysOnTop: boolean
  floatOpacity: number
  floatSnapEdges: boolean
  floatSnapWindows: boolean
  floatSnapDistance: number
  floatSlots: number
  floatDocked: boolean
  floatDockAnchor: 'tray' | 'iconsEnd' | 'iconsStart' | 'widgetsLeft' | 'widgetsRight' | 'free'
  floatDockX: number
  floatDockSlots: number
  floatRing: 'both' | '5h' | 'weekly' | 'tight' | 'none'
  floatSize: number
  floatPosition: [number, number] | null
  floatHotkey: string
  floatHotkeyEnabled: boolean
  floatBadges: boolean
  floatDimIdle: boolean
  floatRunningOnly: boolean
  floatShowOpen: boolean
  floatTips: boolean
  taskbarSeparate: boolean
  taskbarBadge: boolean
  taskbarRelaunchThroughLauncher: boolean
  taskbarTagDefaultInstance: boolean
  taskbarRescanSeconds: number
  usageEnabled: boolean
  usageRefreshMinutes: number
  usageShowReset: boolean
  usageValue: 'left' | 'used'
  usageBarTone: 'calm' | 'accent'
  usageShowPace: boolean
  usageWarnPercent: number
  usageCritPercent: number
  notifyReset: boolean
  notifyLow: boolean
  deleteMovesToTrash: boolean
  deleteProfileByDefault: boolean
  profilesRoot: string | null
  checkUpdates: boolean
  autoInstallUpdates: boolean
  updateHour: number
}

export type UpdateState =
  | { status: 'unsupported'; reason: 'dev' | 'portable' | 'package' | 'os' }
  | { status: 'idle' }
  | { status: 'checking' }
  | { status: 'latest'; checkedAt: string }
  | { status: 'downloading'; version: string; percent: number; notes: string | null }
  | { status: 'ready'; version: string; notes: string | null }
  | { status: 'installing' }
  | { status: 'error'; message: 'network' | 'check' | 'download' | 'install'; checkedAt: string }

export interface Brand {
  color: string
  letter: string
  logo: string
}

export interface Installation {
  id: string
  adapter: 'claude' | 'electron' | 'chromium' | 'webapp' | 'custom' | string
  name: string
  exe: string
  version: string | null
  source: 'detected' | 'manual'
  options: Record<string, unknown>
  present: boolean
  hidden: boolean
  brand: Brand
  isolation: 'userDataDir' | 'none'
  hasUsage: boolean
  supportsDefaultProfile: boolean
  defaultProfileDir: string | null
}

export type Profile = { kind: 'default' } | { kind: 'dir'; path: string; managed: boolean }

export interface LaunchOptions {
  args: string[]
  env: [string, string][]
  url: string | null
  autostart: boolean
  startMinimized: boolean
}

export interface InstanceAdvanced {
  taskbarSeparate: Tristate
  taskbarBadge: Tristate
  relaunchThroughLauncher: Tristate
  usageEnabled: Tristate
  usageOrgs: string[]
  hiddenInQuick: boolean
  hiddenInFloat: boolean
}

export type QuotaKind = 'fiveHour' | 'weekly' | 'monthly' | 'scoped' | 'credits'

export interface Quota {
  kind: QuotaKind
  label: string
  usedPercent: number
  resetsAt: string | null
  usedAmount: number | null
  limitAmount: number | null
  currency: string | null
}

export interface OrgUsage {
  orgUuid: string
  orgName: string
  orgType: string
  plan: string
  quotas: Quota[]
  breakdown: { label: string; percent: number }[]
  unknownFields: string[]
}

export interface UsageReport {
  accountUuid: string
  email: string
  orgs: OrgUsage[]
  fetchedAt: string
}

export type UsageError =
  | { code: 'notSignedIn' }
  | { code: 'accountUnknown' }
  | { code: 'sessionExpired' }
  | { code: 'decrypt'; detail: string }
  | { code: 'rateLimited'; detail: number }
  | { code: 'http'; detail: number }
  | { code: 'network'; detail: string }
  | { code: 'format'; detail: string }
  | { code: 'unsupported' }

export interface UsageEntry {
  status: 'idle' | 'loading' | 'ok' | 'error'
  report: UsageReport | null
  error: UsageError | null
  nextAt: string
  source: string
}

export interface Instance {
  id: string
  installationId: string
  alias: string
  color: string
  glyph: string | null
  profile: Profile
  groupId: string | null
  note: string
  launch: LaunchOptions
  advanced: InstanceAdvanced
  order: number
  favorite: boolean
  createdAt: string
  lastLaunchedAt: string | null
  launchCount: number
  effective: {
    taskbarSeparate: boolean
    taskbarBadge: boolean
    relaunchThroughLauncher: boolean
    usageEnabled: boolean
  }
  running: boolean
  pids: number[]
  profileDir: string | null
  profileExists: boolean
  usage: UsageEntry | null
  /** Organisations shown by default: the instance's own pick, else the ones no other instance
   *  signed in to the same account already stands for (decided by the launcher). */
  shownOrgs: string[]
  appId: string
  taskbarNeedsRestart: boolean
}

export interface Group {
  id: string
  name: string
  order: number
}

export interface Preset {
  id: string
  name: string
  url: string
  logo: string
  color: string
}

export interface Snapshot {
  version: string
  os: 'windows' | 'linux' | 'mac'
  settings: Settings
  groups: Group[]
  installations: Installation[]
  instances: Instance[]
  presets: Preset[]
  configDir: string
  update: UpdateState
  availableWebApps: { appId: string; name: string; profile: string }[]
}
