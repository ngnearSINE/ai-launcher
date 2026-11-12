# Changelog

## 0.3.0

A reworked floating bar that can also live in the taskbar.

- The floating bar stays on screen while the launcher runs and snaps to screen and window edges
- Resizing: drag a corner across the bar for icon size, along it for how many icons show. The long sides change the icon size only, the ends the count only (swapped on a horizontal bar). Up to 5× larger
- Right-click the handle to switch horizontal / vertical. A used-up limit draws its ring as a red dashed track
- Drop the bar on the taskbar to dock it. A translucent ghost shows exactly where it will go before you let go
- Docked, drag the handle sideways to move it; it snaps to the end of the app icons, either side of the widgets and the notification area, and keeps that place as apps open and close
- Drag the docked bar out of the taskbar: a ghost shows where it will float, and it lands there when you let go
- Drag either end of the docked bar for more or fewer icons (remembered apart from the floating count). Clicking, right-clicking, scrolling and reordering icons work as on the floating bar
- Windows other apps dock into the taskbar (such as the AI usage widget) are left uncovered
- Sidebar "Group by" now starts with "None", the default; "Show in rows" switches weekly and 5-hour usage separately
- The title bar shows an update notice instead of search when a new version is out; the overview has a Fable column
- Fixed: the "almost at the limit" alert repeating, and settings not loading from a store file that starts with a BOM

## 0.2.1

- When one account (e.g. a work email with both a team and a personal organisation) is signed in to several instances, an instance with no organisation picked now leaves out the ones another instance already shows. A personal instance set up after a team one no longer shows the same usage. This applies to the sidebar, details, floating launcher, alerts and the suggested account; "Organisations to show" in the edit panel marks the automatic choice

## 0.2.0

A redesigned main window.

- A sidebar and a single pane instead of the card grid: instances grouped by status (needs attention, running low, running, idle) with pinned ones on top. Grouping, sorting and what each row shows live under "Display" at the bottom of the sidebar; on a narrow window it folds to an icon rail
- Overview: the best Claude account to use now and usage per account. Select an instance to see its usage, memory and folder
- Command palette (`Ctrl+K`): launch or switch to any instance and run any command; the tray quick launch is the same list
- Shortcuts: `Ctrl+1`–`9` (hold `Ctrl` to see the numbers), `↑` `↓`, `Ctrl+B` to fold the sidebar, `?` for every shortcut. In-window and global shortcuts are remappable in Settings → Shortcuts
- Usage alerts now actually fire: when an account runs low it names one with room, and it tells you when a limit resets
- Themes: system, light, dark, Claude, Midnight and pure black; the window backdrop follows the theme
- Status badges on icons (needs attention, warning, running, starting), one notification at a time, and deletes can be undone for 6 seconds (`Ctrl+Z`)
- Minimising the main window switches to the floating launcher (on by default; turn it off in Settings to get the title-bar button back)
- Fixed: a window hidden while minimised reopened tiny and off-screen

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
