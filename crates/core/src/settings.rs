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
    /// Least-left percentage at the end of each sidebar row.
    pub sidebar_show_usage: bool,
    /// In-window shortcut overrides by binding id (`""` = unassigned). Defaults live in the UI.
    pub keymap: std::collections::BTreeMap<String, String>,

    // Launcher behaviour
    pub start_with_os: bool,
    pub start_hidden: bool,
    pub close_to_tray: bool,
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

    // Floating launcher
    pub float_enabled: bool,
    pub float_orientation: String,
    pub float_always_on_top: bool,
    pub float_opacity: u8,
    pub float_snap_edges: bool,
    /// Usage ring around each icon: `both` (outer weekly, inner 5-hour), `5h`, `weekly`,
    /// `tight` (whichever has less left) or `none`.
    pub float_ring: String,
    pub float_size: u16,
    pub float_position: Option<(i32, i32)>,
    /// Main window ⇄ floating launcher: `swap` hides one when showing the other, `both` keeps both.
    pub float_switch: String,
    /// Minimising the main window switches to the floating launcher (and the title bar drops
    /// its separate floating button).
    pub minimize_to_float: bool,
    pub float_hotkey: String,
    pub float_hotkey_enabled: bool,

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
            sidebar_group: "status".into(),
            sidebar_show_account: false,
            sidebar_show_usage: true,
            keymap: Default::default(),

            start_with_os: false,
            start_hidden: true,
            close_to_tray: true,
            focus_if_running: true,
            hide_after_launch: false,
            confirm_quit_instance: true,
            quick_hotkey: "Alt+Shift+Space".into(),
            quick_hotkey_enabled: true,
            best_claude_hotkey: "Alt+Shift+C".into(),
            best_claude_hotkey_enabled: false,
            tray_left_click: "popup".into(),
            tray_double_click: "main".into(),

            float_enabled: false,
            float_orientation: "vertical".into(),
            float_always_on_top: true,
            float_opacity: 92,
            float_snap_edges: true,
            float_ring: "both".into(),
            float_size: 44,
            float_position: None,
            float_switch: "swap".into(),
            minimize_to_float: true,
            float_hotkey: "Alt+Shift+F".into(),
            float_hotkey_enabled: true,

            taskbar_separate: true,
            taskbar_badge: true,
            taskbar_relaunch_through_launcher: true,
            taskbar_tag_default_instance: false,
            taskbar_rescan_seconds: 2,

            usage_enabled: true,
            usage_refresh_minutes: 5,
            usage_show_reset: true,
            usage_value: "left".into(),
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

impl Settings {
    /// Clamp values a hand-edited file or an old version may have left out of range.
    pub fn sanitize(&mut self) {
        self.glass_strength = self.glass_strength.min(100);
        self.float_opacity = self.float_opacity.clamp(30, 100);
        self.float_size = self.float_size.clamp(28, 96);
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
