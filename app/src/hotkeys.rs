//! Global hotkeys: the quick panel and "launch the Claude instance with the most headroom".

use std::sync::Arc;

use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::core::Core;

pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state != ShortcutState::Pressed {
                return;
            }
            let Some(core) = app.try_state::<Arc<Core>>() else { return };
            let s = core.settings();
            if s.quick_hotkey_enabled && parse(&s.quick_hotkey).as_ref() == Some(shortcut) {
                crate::windows::toggle_quick(app, None);
            } else if s.float_hotkey_enabled && parse(&s.float_hotkey).as_ref() == Some(shortcut) {
                crate::windows::toggle_main(app);
            } else if s.best_claude_hotkey_enabled && parse(&s.best_claude_hotkey).as_ref() == Some(shortcut) {
                if let Err(e) = core.launch_best_claude() {
                    log::warn!("best claude: {e}");
                }
            }
        })
        .build()
}

fn parse(s: &str) -> Option<Shortcut> {
    s.parse::<Shortcut>().ok()
}

/// Re-registers hotkeys from settings. Failures (taken by another app) are logged; the settings
/// screen shows whether each one is active.
pub fn apply(app: &AppHandle, core: &Arc<Core>) {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let s = core.settings();
    for (on, key) in [
        (s.quick_hotkey_enabled, &s.quick_hotkey),
        (s.best_claude_hotkey_enabled, &s.best_claude_hotkey),
        (s.float_hotkey_enabled, &s.float_hotkey),
    ] {
        if !on {
            continue;
        }
        match parse(key) {
            Some(sc) => {
                if let Err(e) = gs.register(sc) {
                    log::warn!("hotkey {key}: {e}");
                }
            }
            None => log::warn!("hotkey {key}: cannot parse"),
        }
    }
}
