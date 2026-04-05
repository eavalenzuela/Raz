# Raz

A minimal personal launcher and homepage built with Rust, Tauri v2, and Svelte. Launch apps, bookmark links, manage local servers, and monitor system status — all from one clean desktop tool.

## Features

- **Apps** — Launch local applications and games with configurable paths, arguments, environment variables, and icons. Import from `.desktop` files. Right-click to edit, open directory, or pin to sidebar.
- **Links** — Bookmark URLs with auto-fetched favicons in a draggable grid. Click to open in your browser. Right-click to edit, pin, or remove.
- **Servers** — Manage local server processes (dev servers, game servers, self-hosted services). Start/stop controls, live stdout/stderr log viewer, auto-launch on startup.
- **Status Monitor** — Live HTTP and ping checks in the sidebar with up/down indicators, configurable intervals, and desktop notifications on state changes.
- **System Tray** — Minimize to tray on close, server status visible in the tray menu, click to restore.
- **Settings** — Configure check intervals, notification preferences, and tray behavior from the UI.
- **Theming** — Automatically matches your system light/dark preference.

## Install

Download the latest release from the [Releases page](https://github.com/eavalenzuela/Raz/releases).

**Ubuntu/Debian:**
```bash
sudo dpkg -i Raz_1.0.0_amd64.deb
```

**Fedora/RHEL:**
```bash
sudo rpm -i Raz-1.0.0-1.x86_64.rpm
```

**AppImage (portable):**
```bash
chmod +x Raz_1.0.0_amd64.AppImage
./Raz_1.0.0_amd64.AppImage
```

## Build from Source

Requires [Rust](https://rustup.rs/), [Node.js](https://nodejs.org/), and the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm install
npx tauri build
```

Release artifacts will be in `src-tauri/target/release/bundle/`.

## Data

All configuration is stored in a single JSON file at `~/.config/raz/config.json`, auto-saved on every UI change. No manual editing required.

## License

See [LICENSE](LICENSE) for details.
