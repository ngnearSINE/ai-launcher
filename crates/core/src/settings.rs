use serde::{Deserialize, Serialize};

/// A per-instance switch that can defer to the global setting.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Tristate {
    #[default]
    Inherit,
    On,
    Off,
}

impl Tristate {
    pub fn resolve(self, global: bool) -> bool {
        match self {
            Tristate::Inherit => global,
            Tristate::On => true,
            Tristate::Off => false,
        }
    }
}

/// Everything the user can tune. The default screen exposes only a few of these;
/// the rest live under the advanced tabs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Which one-time migrations ran; missing in old files, so they start at 0.
    #[serde(default)]
    pub settings_rev: u32,
    // Appearance
    pub language: String,
    /// `system`, `light`, `dark`, or a dark variant: `claude`, `midnight`, `black`.
    pub theme: String,
    pub accent: String,
    pub glass: bool,
    /// How much the window background lets the backdrop through (0 = opaque, 100 = clearest).
    pub glass_strength: u8,
    /// Windows backdrop material: `tabbed` (Mica Alt), `mica`, `acrylic` or `none`.
    pub background_effect: String,
    pub reduce_motion: bool,
    pub density: String,
    pub card_usage_style: String,
    pub sort: String,
    /// Old card-grid grouping; kept so older files still load. The sidebar uses `sidebar_group`.
    pub group_by: String,
    /// Sidebar sections: `status` (needs attention → running low → running → idle), `app`,
    /// `group` or `none`.
    pub sidebar_group: String,
    /// Second line under each sidebar row with the signed-in account.
    pub sidebar_show_account: bool,
    /// Usage at the end of each sidebar row: the 5-hour and / or the weekly window.
    pub sidebar_show_5h: bool,
    pub sidebar_show_weekly: bool,
    /// In-window shortcut overrides by binding id (`""` = unassigned). Defaults live in the UI.
    pub keymap: std::collections::BTreeMap<String, String>,

    // Launcher behaviour
    pub start_with_os: bool,
    pub start_hidden: bool,
    pub focus_if_running: bool,
    pub hide_after_launch: bool,
    pub confirm_quit_instance: bool,
    pub quick_hotkey: String,
    pub quick_hotkey_enabled: bool,
    pub best_claude_hotkey: String,
    pub best_claude_hotkey_enabled: bool,
    pub tray_left_click: String,
    /// `main` (default), `quick` or `none`.
    pub tray_double_click: String,

    // Floating launcher (always shown while the launcher runs)
    pub float_orientation: String,
    pub float_always_on_top: bool,
    pub float_opacity: u8,
    /// Magnet: while dragged, the strip clicks onto screen edges (and other windows').
    pub float_snap_edges: bool,
    pub float_snap_windows: bool,
    /// Logical px within which the magnet pulls.
    pub float_snap_distance: u16,
    /// Cells shown along the strip; the rest scroll with the wheel. `0` = all.
    pub float_slots: u16,
    /// Sits inside the taskbar (dropped onto it), laid out horizontally.
    pub float_docked: bool,
    /// What the docked strip stays next to: `tray`, `iconsEnd`, `iconsStart`, `widgetsLeft`,
    /// `widgetsRight` or `free` (then `float_dock_x`, logical px from the taskbar's left).
    pub float_dock_anchor: String,
    pub float_dock_x: i32,
    /// Cells shown while docked, set apart from `float_slots` so docking and undocking never
    /// change the floating strip. `0` = as many as fit.
    pub float_dock_slots: u16,
    /// Usage ring around each icon: `both` (outer weekly, inner 5-hour), `5h`, `weekly`,
    /// `tight` (whichever has less left) or `none`.
    pub float_ring: String,
    /// Scale of the whole strip, as the icon size in px at 100 % (44 = 100 %).
    pub float_size: u16,
    pub float_position: Option<(i32, i32)>,
    /// Global hotkey that opens / closes the main window.
    pub float_hotkey: String,
    pub float_hotkey_enabled: bool,
    /// The instance-colour dot (with its running / usage ring) on each icon's corner.
    pub float_badges: bool,
    /// Icons of instances that are not running are drawn faded.
    pub float_dim_idle: bool,
    /// Only running instances are on the strip.
    pub float_running_only: bool,
    /// The "open the main window" button at the strip's end.
    pub float_show_open: bool,
    /// Hovering an icon shows its account and usage.
    pub float_tips: bool,

    // Taskbar (advanced)
    pub taskbar_separate: bool,
    pub taskbar_badge: bool,
    pub taskbar_relaunch_through_launcher: bool,
    pub taskbar_tag_default_instance: bool,
    pub taskbar_rescan_seconds: u16,

    // Usage
    pub usage_enabled: bool,
    pub usage_refresh_minutes: u16,
    pub usage_show_reset: bool,
    /// Numbers and bars show what is `left` (default) or what is `used`.
    pub usage_value: String,
    /// Bar fill: `calm` (neutral, colour only when running low — default) or `accent`.
    pub usage_bar_tone: String,
    /// White tick where usage should be if it were spread evenly over the window.
    pub usage_show_pace: bool,
    pub usage_warn_percent: u8,
    pub usage_crit_percent: u8,
    pub notify_reset: bool,
    pub notify_low: bool,

    // Data
    pub delete_moves_to_trash: bool,
    pub delete_profile_by_default: bool,
    pub profiles_root: Option<String>,

    // Updates
    /// Check GitHub Releases in the background and download new versions.
    pub check_updates: bool,
    /// Install a downloaded update during the nightly hour without asking.
    pub auto_install_updates: bool,
    /// Local hour (0–23) of the nightly install window.
    pub update_hour: u8,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            settings_rev: SETTINGS_REV,
            language: "system".into(),
            theme: "system".into(),
            accent: "#0A84FF".into(),
            glass: true,
            glass_strength: 45,
            background_effect: "tabbed".into(),
            reduce_motion: false,
            density: "comfortable".into(),
            card_usage_style: "bars".into(),
            sort: "manual".into(),
            group_by: "none".into(),
            sidebar_group: "none".into(),
            sidebar_show_account: false,
            sidebar_show_5h: true,
            sidebar_show_weekly: true,
            keymap: Default::default(),

            start_with_os: false,
            start_hidden: true,
            focus_if_running: true,
            hide_after_launch: false,
            confirm_quit_instance: true,
            quick_hotkey: "Alt+Shift+Space".into(),
            quick_hotkey_enabled: true,
            best_claude_hotkey: "Alt+Shift+C".into(),
            best_claude_hotkey_enabled: false,
            tray_left_click: "popup".into(),
            tray_double_click: "main".into(),

            float_orientation: "vertical".into(),
            float_always_on_top: true,
            float_opacity: 92,
            float_snap_edges: true,
            float_snap_windows: true,
            float_snap_distance: 12,
            float_slots: 0,
            float_docked: false,
            float_dock_anchor: "tray".into(),
            float_dock_x: 0,
            float_dock_slots: 0,
            float_ring: "both".into(),
            float_size: 44,
            float_position: None,
            float_hotkey: "Alt+Shift+F".into(),
            float_hotkey_enabled: true,
            float_badges: true,
            float_dim_idle: false,
            float_running_only: false,
            float_show_open: true,
            float_tips: true,

            taskbar_separate: true,
            taskbar_badge: true,
            taskbar_relaunch_through_launcher: true,
            taskbar_tag_default_instance: false,
            taskbar_rescan_seconds: 2,

            usage_enabled: true,
            usage_refresh_minutes: 5,
            usage_show_reset: true,
            usage_value: "left".into(),
            usage_bar_tone: "calm".into(),
            usage_show_pace: true,
            usage_warn_percent: 70,
            usage_crit_percent: 90,
            notify_reset: false,
            notify_low: true,

            delete_moves_to_trash: true,
            delete_profile_by_default: false,
            profiles_root: None,
            check_updates: true,
            auto_install_updates: true,
            update_hour: 3,
        }
    }
}

/// Floating bar size range (`float_size`, 44 = 100 %). The top is the web view's own zoom
/// limit (5×): beyond it the icons could not follow the window any more.
pub const FLOAT_SIZE_MIN: u16 = 28;
pub const FLOAT_SIZE_MAX: u16 = 220;

/// One-time changes applied to settings saved by older versions (see [`Settings::migrate`]).
pub const SETTINGS_REV: u32 = 1;

impl Settings {
    /// Brings settings saved by an older version up to date, once.
    /// 1: the sidebar is no longer grouped by default (Leonard, 2026-11-12).
    pub fn migrate(&mut self) {
        if self.settings_rev < 1 {
            self.sidebar_group = "none".into();
        }
        self.settings_rev = SETTINGS_REV;
    }

    /// Clamp values a hand-edited file or an old version may have left out of range.
    pub fn sanitize(&mut self) {
        self.glass_strength = self.glass_strength.min(100);
        self.float_opacity = self.float_opacity.clamp(30, 100);
        self.float_size = self.float_size.clamp(FLOAT_SIZE_MIN, FLOAT_SIZE_MAX);
        self.float_snap_distance = self.float_snap_distance.clamp(2, 48);
        if !["tray", "iconsEnd", "iconsStart", "widgetsLeft", "widgetsRight", "free"].contains(&self.float_dock_anchor.as_str()) {
            self.float_dock_anchor = "tray".into();
        }
        self.float_dock_x = self.float_dock_x.max(0);
        self.usage_refresh_minutes = self.usage_refresh_minutes.clamp(2, 240);
        self.taskbar_rescan_seconds = self.taskbar_rescan_seconds.clamp(1, 30);
        self.usage_warn_percent = self.usage_warn_percent.min(100);
        self.usage_crit_percent = self.usage_crit_percent.clamp(self.usage_warn_percent, 100);
        self.update_hour = self.update_hour.min(23);
        if !["system", "light", "dark", "claude", "midnight", "black"].contains(&self.theme.as_str()) {
            self.theme = Settings::default().theme;
        }
        if !["status", "app", "group", "none"].contains(&self.sidebar_group.as_str()) {
            self.sidebar_group = Settings::default().sidebar_group;
        }
        if !self.accent.starts_with('#') || self.accent.len() != 7 {
            self.accent = Settings::default().accent;
        }
    }
}
