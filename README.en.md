# AI Launcher

Run several accounts of Claude Desktop and other AI apps side by side, on Windows and Ubuntu (x64 and arm64).

- **One instance per account** — each instance has its own data folder, so sign-ins never mix.
- **Pin each one separately** (Windows) — every instance gets its own taskbar button; a pinned button starts exactly that instance.
- **Usage cards** — Claude 5-hour, weekly, per-model (e.g. Fable) and credit usage, read **only from that instance's own sign-in**.
- **Auto-detect + add by path** — Claude, Orca, Cursor, Poe and Grok Bot in their default locations, plus AI web apps installed from Chrome, are registered automatically; anything else can be added by path.
- **Web apps** — web apps you installed from Chrome (Gemini, Grok…) open as the default instance with your usual profile; other accounts get app windows with their own browser data folder. Chrome and Edge themselves are not shown as cards.
- **Quick launch, tray and floating launcher** — global hotkey (`Alt+Shift+Space`) → type → Enter; launch from the tray menu; or switch to an always-on-top icon strip with usage rings (title-bar button or `Alt+Shift+F`).

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
3. Start it, right-click its taskbar button → **Pin to taskbar**. A Start-menu shortcut made from the card menu shares the same button.

Taskbar behaviour (separate or merged buttons, colour badges, how pinned buttons launch) lives in **Settings → Taskbar** and can be overridden per instance under **Edit → Advanced**.

## Privacy

Sign-ins stay encrypted inside each profile folder. The launcher decrypts them on this PC only when reading usage (Windows DPAPI, Linux keyring) and never stores or logs them. The only network calls are the usage and profile endpoints of `api.anthropic.com`; usage lookup can be turned off.

## Build

Rust stable and Node.js 20+. On Windows: `.\local-build.ps1 -Test` (exe) or `-Bundle` (installer).

## License

MIT
