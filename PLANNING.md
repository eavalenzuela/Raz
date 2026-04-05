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

## Future Considerations

- Global hotkey / Spotlight-style summoning
- Search bar or command palette
- Sub-grouping or categories within tabs
- User-created custom tabs
- Plugin/extension system
- Multi-machine sync
