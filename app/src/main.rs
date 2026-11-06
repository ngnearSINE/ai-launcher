// Hide the console window of release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod commands;
mod core;
mod hotkeys;
mod i18n;
mod icons;
mod logger;
mod monitor;
mod shortcuts;
mod snapshot;
#[cfg(windows)]
mod taskbar;
mod tray;
mod usage;
mod windows;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};

use crate::core::Core;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = cli::parse(&args);
    logger::init(&launcher_core::Env::detect().config.join("AI Launcher"));
    log::info!("start {} args={args:?}", env!("CARGO_PKG_VERSION"));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            log::info!("forwarded argv={argv:?}");
            let cmd = cli::parse(&argv.into_iter().skip(1).collect::<Vec<_>>());
            // This callback runs inside a WM_COPYDATA send; outgoing cross-process calls made
            // there fail (starting a Store app alias returns os error 1920). Handle it elsewhere.
            let app = app.clone();
            std::thread::spawn(move || {
                if let Some(core) = app.try_state::<Arc<Core>>() {
                    cli::handle(&app, core.inner(), &cmd, false);
                }
            });
        }))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))
        .plugin(hotkeys::plugin())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(commands::handler())
        .setup(move |app| {
            let core = Arc::new(Core::new(app.handle().clone())?);
            app.manage(core.clone());
            core.sync_catalog();
            monitor::start(core.clone());
            usage::start(core.clone());
            #[cfg(windows)]
            taskbar::start_watcher(core.clone());
            tray::build(app.handle(), &core)?;
            hotkeys::apply(app.handle(), &core);
            windows::apply_appearance(app.handle(), &core);
            windows::sync_float(app.handle(), &core);
            core.sync_autostart();
            cli::handle(app.handle(), &core, &first, true);
            Ok(())
        })
        .on_window_event(|window, event| windows::on_event(window, event))
        .build(tauri::generate_context!())
        .expect("failed to start AI Launcher")
        .run(|app, event| {
            // Keep running in the tray when every window is closed.
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            let _ = app;
        });
}

#[allow(dead_code)]
fn _assert_send_sync() {
    fn f<T: Send + Sync>() {}
    f::<Core>();
    let _ = WindowEvent::Focused(true);
}
