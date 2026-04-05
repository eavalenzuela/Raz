# Raz — Application Design

## Stack

- **Backend:** Rust
- **Framework:** Tauri v2
- **Frontend:** Svelte
- **Data:** JSON config file, auto-saved on every UI change

## Core Concept

A minimal personal launcher and homepage. Not a power-user dashboard — a clean, focused tool for launching apps, bookmarking links, and monitoring system/service status.

## Layout

```
┌──────────────────────────────────────────────────┐
│ [≡]                                              │
├────────────┬───────────┬───────────┬─────────────┤
│ Apps       │ Links     │ Servers   │  Pinned     │
├────────────┴───────────┴───────────┤  Items      │
│                                    ├─────────────┤
│                                    │             │
│         Main content area          │  Status     │
│                                    │  Monitor    │
│                                    │             │
│                                    │             │
│                                    │             │
├────────────────────────────────────┴─────────────┤
│ ● All systems online · Last checked: 2m ago      │
└──────────────────────────────────────────────────┘
```

## Architecture

### Window
- Standard desktop window with title bar
- Minimize-to-tray on close (configurable in Settings)
- System tray icon with server status menu and restore/quit actions

### Menu Button (top-left)
- Hamburger icon dropdown: Settings, Create Desktop Icon, About, Quit
- Settings modal for app-wide configuration
- All configuration through the UI — no manual file editing

### Tabs
- **Apps** — Launchable local applications with name, executable, args, working dir, env vars, icon, type label. Simple mode or raw shell command mode. Import from `.desktop` files. Right-click: edit, open directory, pin to sidebar, remove.
- **Links** — Bookmarked URLs with favicon grid. Drag-and-drop reordering. Right-click: edit, pin to sidebar, remove.
- **Servers** — Managed local processes with start/stop, live stdout/stderr log viewer, auto-launch on startup. Right-click: edit, open directory, view logs, remove.

### Right Sidebar (collapsible)
- **Pinned Items** — Quick-access shortcuts to any app or link, visible from all tabs
- **Status Monitor** — Live HTTP/ping checks with up/down/unknown indicators, configurable intervals, desktop notifications on state changes

### Status Bar
- Glanceable summary: overall health + last check timestamp

### Settings
- Default check interval for status monitors
- Notification preferences (enable/disable, per-transition toggles)
- Minimize-to-tray toggle

### Data
- Single JSON config at `~/.config/raz/config.json`, auto-saved on every change

### Packaging
- `.deb` (Ubuntu/Debian), `.rpm` (Fedora/RHEL), `.AppImage` (portable)
- `.desktop` file registered via package or "Create Desktop Icon" menu action
- System tray via Tauri tray API

## Roadmap

### Apps
- [x] Search/filter bar — type to filter apps by name or type label
- [x] Drag-and-drop reordering — backend `reorder_apps` exists, wire up in UI
- [x] Grid view toggle — switch between list view and compact icon grid
- [x] Launch counter / recents — track launch count and last-used timestamp, sort by most-used or recent
- [x] Bulk .desktop import — scan `/usr/share/applications` and pick multiple apps to import at once

### Links
- [x] Folders / collections — group links into named folders with collapsible sections
- [x] Local favicon caching — download and store favicons locally (offline support + privacy)
- [x] URL validation & metadata fetch — on add, fetch page title as default name and verify URL is reachable

### Servers
- [x] Auto-restart on crash — configurable per server, with optional max-retry count and cooldown delay
- [x] Log timestamps — prepend each output line with a timestamp
- [x] Log search/filter — text filter or regex search within the log viewer, with highlight
- [x] Log export — button to save the current log buffer to a file
- [x] Resource monitoring — show PID, uptime, and CPU/memory usage per running server (via `/proc` on Linux)

### Future Considerations
- Global hotkey / Spotlight-style summoning
- Plugin/extension system
- Multi-machine sync
