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

### Window Behavior
- Standard desktop window with title bar
- Minimize-to-tray instead of closing (system tray icon to restore)
- Installs as a `.desktop` entry so it appears in the Ubuntu Apps launcher

### Menu Button (top-left)
- Hamburger icon (or Raz app icon) — not a text label
- Dropdown includes Settings, About, Quit, etc.
- Settings opens a modal for app-wide configuration
- All configuration happens through the UI — no manual file editing required

### Tab Bar
- Sits below the menu button, spans the width of the main content area (not the sidebar)
- Default tabs: **Apps**, **Links**, **Servers**
- No user-created tabs for now

### Main Content Area
- Left ~75% of the window
- Displays the active tab's content

### Right Sidebar (collapsible)
- Right ~25% of the window, with a toggle or drag handle to collapse
- **Pinned Items** (top section): Favorite apps/links for one-click access from any tab
- **Status Monitor** (bottom section): Live status indicators, always visible regardless of active tab

### Bottom Status Bar
- Thin bar spanning full window width
- Glanceable summary: overall health ("All systems online" / "2 of 5 targets down"), last check timestamp

### Theming
- Match the system theme (light/dark) automatically via OS preference detection

## Tabs

Each tab is a dedicated view — no internal sub-grouping within tabs for now.

### Apps
- List of launchable local applications and games
- Each entry stores: name, executable path, optional icon, optional type label (e.g. "game", "tool"), environment config (e.g. `.venv` path), arguments, working directory
- Click to launch
- Right-click context menu: edit config, open containing directory, pin to sidebar, remove
- Add new entries via a button/form within the tab

### Links
- Bookmarked URLs with preview thumbnails
- Click to open in external browser
- Right-click context menu: edit, pin to sidebar, remove
- Add/remove/reorder via UI controls
- Drag-and-drop reordering

### Servers
- Managed local server processes (e.g. dev servers, game servers, self-hosted services)
- Each entry stores: name, executable path, arguments, working directory, environment config, optional auto-launch on Raz startup
- Start/stop controls per server
- Live CLI output panel — click a server to view its stdout/stderr in a log viewer area within the tab
- Visual state per server: running, stopped, crashed
- Auto-launch: optionally start specific servers when Raz opens
- Right-click context menu: edit config, open containing directory, view logs, remove
- Add new entries via a button/form within the tab

## Right Sidebar

### Pinned Items
- Quick-access shortcuts to any app or link, pinned via right-click context menu
- Visible from any tab
- Click to launch app / open link

### Status Monitor
- Live status indicators for servers, devices, and webpages
- Each entry stores: name, target (IP/hostname/URL), check type (ping/HTTP), check interval
- Visual state: up, down, unknown/pending
- Desktop notifications on status changes (up→down, down→up)
- Add/edit/remove entries via the sidebar itself (e.g. a "+" button and right-click context menu)

## Settings (modal via menu button)

- Default check interval for status monitors
- Notification preferences (enable/disable, which transitions to alert on)
- Tray behavior (minimize to tray on close — on/off)
- Any other app-wide preferences as they arise

## Data & Persistence

- Single JSON config file stored in an appropriate XDG location (e.g. `~/.config/raz/config.json`)
- Auto-saved whenever the user makes a change through the UI
- No import/export — the file is there if someone wants to back it up manually, but it's not a supported workflow

## Installation

- Builds to a `.deb` package (primary target: Ubuntu)
- Registers a `.desktop` file for the system launcher
- System tray integration via Tauri's tray API

## Implementation Status

### Complete
- **Layout & shell:** Toolbar, tab bar, main content area, sidebar, status bar — all wired up
- **Apps tab:** List, click-to-launch, add/edit modal (simple + command modes), icon support, .desktop import, right-click context menu (edit, open directory, pin to sidebar, remove)
- **Links tab:** Grid with favicons, click to open in browser, add/edit modal, right-click context menu (edit, pin to sidebar, remove)
- **Servers tab:** List with status dots, start/stop controls, live stdout/stderr log viewer, add/edit modal (simple + command modes), auto-launch flag, right-click context menu (edit, remove)
- **Sidebar:** Collapsible with toggle button, pinned items (click to launch/open, unpin), status monitor (add/edit/remove via context menu, live up/down/unknown dots, last-check timestamps)
- **Status bar:** Summary ("All systems online" / "X of Y targets down"), last check timestamp
- **Menu button:** Hamburger icon, dropdown with Settings / About / Quit
- **System tray:** Tray icon with server status menu, left-click to restore, minimize-to-tray on close, Quit from tray menu
- **Theming:** Auto light/dark via `prefers-color-scheme` media query
- **Desktop notifications:** Status monitor fires notifications on state changes via `@tauri-apps/plugin-notification`
- **Auto-launch servers** on startup
- **Settings modal:** Controls for default check interval, notification preferences (enabled, on-down, on-up), minimize-to-tray toggle — all persisted
- **About dialog:** Version, description, and tech stack info
- **Servers context menu:** Edit, Open Directory, View Logs, Remove

### Remaining
- **Drag-and-drop reordering** for links
- **`.deb` packaging** and `.desktop` file registration

## Out of Scope (for now)

- Global hotkey / Spotlight-style summoning
- Search bar or command palette
- Sub-grouping or categories within tabs
- User-created custom tabs
- Plugin/extension system
- Multi-machine sync
