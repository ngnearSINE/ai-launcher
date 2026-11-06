# Changelog

## 0.1.0

First public release.

- Run several accounts of Claude Desktop, Electron apps (Orca, Cursor, Poe…) and web apps (Gemini, Grok, ChatGPT…) side by side, each with its own data folder. Web apps installed from Chrome open as the default instance
- Auto-detects installed apps, adds programs by path, adopts existing `Claude-*` profiles
- Windows: a separate taskbar button per instance that can be pinned (colour badge icon; a pinned button starts exactly that instance). Separate/merge and badges in Settings → Taskbar and per instance under Advanced
- Claude usage cards: 5-hour, weekly, per-model, credits and this week's split — read only from each instance's own sign-in
- Tray (click: quick launch, double-click: main window), global quick-launch hotkey, always-on-top floating launcher (usage rings: 5-hour, weekly, both, or the tighter one); switch with the title-bar button or `Alt+Shift+F`
- Drag cards and floating icons to reorder (with a lifted preview), groups, favourites, search, sorting, desktop/Start-menu shortcuts
- Installers for Windows and Ubuntu on both x64 and arm64
- Installer and automatic updates (checks every 6 hours; installs overnight or on "Restart to update")
- Ubuntu: app icons from the desktop entries, Cursor AppImage and Grok Desktop detected too
- Korean and English, light and dark, glass effect
