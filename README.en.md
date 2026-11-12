# AI Launcher

Run several accounts of Claude Desktop and other AI apps side by side, on Windows and Ubuntu (x64 and arm64).

- **One instance per account** — each instance has its own data folder, so sign-ins never mix.
- **Pin each one separately** (Windows) — every instance gets its own taskbar button; a pinned button starts exactly that instance.
- **Sidebar and overview** — pinned instances on top, each row with its 5-hour and weekly usage; group by status or app if you like. The overview shows the best Claude account to use now and usage per account (5-hour, weekly, Fable); an instance's page shows its usage, memory and folder. An available update shows in the title bar.
- **Usage** — Claude 5-hour, weekly, per-model (e.g. Fable) and credit usage with time to reset, and a notification naming an account with room when one runs low. Read **only from that instance's own sign-in**.
- **Command palette** — `Ctrl+K` finds any instance or command; the tray quick launch is the same list. Press `?` for every shortcut (remappable in Settings → Shortcuts).
- **Auto-detect + add by path** — Claude, Orca, Cursor, Poe and Grok Bot in their default locations, plus AI web apps installed from Chrome, are registered automatically; anything else can be added by path.
- **Web apps** — web apps you installed from Chrome (Gemini, Grok…) open as the default instance with your usual profile; other accounts get app windows with their own browser data folder. Chrome and Edge themselves are not shown as cards.
- **Quick launch, tray and floating bar** — global hotkey (`Alt+Shift+Space`) → type → Enter, or launch from the tray menu. While the launcher runs an always-on-top icon strip with usage rings stays on screen and snaps to screen and window edges as you drag it. Drag a corner to resize it, an end to show more or fewer icons (scroll for the rest); right-click an icon to hide it (bring it back from the main window). Drop it on the taskbar to dock it there horizontally; drag its handle to take it out. Switch horizontal / vertical from its right-click menu. The button at the bottom or `Alt+Shift+F` opens and closes the main window.

## Supported apps

| Kind | Examples | Isolation |
| --- | --- | --- |
| Claude Desktop | Windows Store / installer builds, Ubuntu `claude-desktop` | `--user-data-dir` |
| Electron apps | Orca, Cursor (incl. the Ubuntu AppImage), Poe, Grok Bot, Grok Desktop (Ubuntu), any Electron app added by path | `--user-data-dir` |
| Web apps | Chrome-installed web apps (Gemini, Grok, ChatGPT…), Claude web, Perplexity, custom URL | default: the installed web app as is; extra accounts: browser data folder + app window |
| Other programs | added by path | launch only (opt in to `--user-data-dir`) |

## Usage

1. On first start the launcher finds installed apps and shows one default instance (your usual sign-in) per app.
2. **New instance** → pick the app, a name and a colour. It starts in a fresh folder; sign in with another account.
3. Start it, right-click its taskbar button → **Pin to taskbar**. A Start-menu shortcut made from the instance menu (⋯ or right-click) shares the same button.

Taskbar behaviour (separate or merged buttons, colour badges, how pinned buttons launch) lives in **Settings → Taskbar** and can be overridden per instance under **Edit → Advanced**.

## Privacy

Sign-ins stay encrypted inside each profile folder. The launcher decrypts them on this PC only when reading usage (Windows DPAPI, Linux keyring) and never stores or logs them. The only network calls are the usage and profile endpoints of `api.anthropic.com`; usage lookup can be turned off.

## Build

Rust stable and Node.js 20+. On Windows: `.\local-build.ps1 -Test` (exe) or `-Bundle` (installer).

## License

MIT
