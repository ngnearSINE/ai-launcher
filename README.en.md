# AI Launcher

Run several accounts of Claude Desktop and other AI apps side by side, on Windows and Ubuntu.

- **One instance per account** — each instance has its own data folder, so sign-ins never mix.
- **Pin each one separately** (Windows) — every instance gets its own taskbar button; a pinned button starts exactly that instance.
- **Usage cards** — Claude 5-hour, weekly, per-model (e.g. Fable) and credit usage, read **only from that instance's own sign-in**.
- **Auto-detect + add by path** — Claude, Chrome, Edge, Orca and Cursor in their default locations are registered automatically; anything else can be added by path.
- **Web apps** — Gemini, Grok, ChatGPT and others as app windows, each with its own browser data folder.
- **Quick launch, tray and floating launcher** — global hotkey (`Alt+Shift+Space`) → type → Enter; launch from the tray menu; optional always-on-top icon strip.

## Supported apps

| Kind | Examples | Isolation |
| --- | --- | --- |
| Claude Desktop | Windows Store / installer builds, Ubuntu `claude-desktop` | `--user-data-dir` |
| Electron apps | Orca, Cursor, Grok Bot, any Electron app added by path | `--user-data-dir` |
| Chromium browsers | Chrome, Edge, Brave, Chromium | `--user-data-dir` |
| Web apps | Gemini, Grok, ChatGPT, Claude web, Perplexity, custom URL | browser data folder + app window |
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
