//! Command line: `ai-launcher [launch <instance-id>] [quick] [show] [--hidden] [--autostart]`.
//! Pinned taskbar buttons and generated shortcuts run `launch <id>`; a second process forwards
//! its arguments to the running launcher through the single-instance plugin.

use std::sync::Arc;

use tauri::AppHandle;

use crate::core::Core;

#[derive(Debug, Clone, Default)]
pub struct Cli {
    pub launch: Option<String>,
    pub quick: bool,
    pub show: bool,
    pub hidden: bool,
    pub autostart: bool,
}

pub fn parse(args: &[String]) -> Cli {
    let mut c = Cli::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "launch" | "--launch" => c.launch = it.next().cloned(),
            "quick" | "--quick" => c.quick = true,
            "show" | "--show" => c.show = true,
            "--hidden" | "--minimized" => c.hidden = true,
            "--autostart" => {
                c.autostart = true;
                c.hidden = true;
            }
            _ => {}
        }
    }
    c
}

/// `first` is true for the process's own arguments at start-up.
pub fn handle(app: &AppHandle, core: &Arc<Core>, cmd: &Cli, first: bool) {
    if cmd.autostart {
        core.launch_autostart_instances();
    }
    if let Some(id) = &cmd.launch {
        if let Err(e) = core.launch(id) {
            log::warn!("launch {id}: {e}");
            crate::windows::show_main(app);
        }
        return;
    }
    if cmd.quick {
        crate::windows::show_quick(app, None);
        return;
    }
    let start_hidden = first && (cmd.hidden || core.settings().start_hidden && cmd.autostart);
    if cmd.show || !start_hidden {
        crate::windows::show_main(app);
    }
}
