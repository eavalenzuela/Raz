# Planned Improvements & Features

## Improvements

1. **Reap launched app processes and detach them into their own process group** — `launch_app` drops the `Child` handle, so every launched app that exits becomes a zombie for the lifetime of Raz; wait on it from a background thread and `process_group(0)` so launches don't die with Raz's terminal.
2. **Reset the auto-restart retry counter after a stable run** — a server that crashed `max_retries` times over weeks is permanently denied auto-restart; treat a run ≥60s as stability and reset the counter when it exits.
3. **Strip ANSI escape sequences from server log lines** — dev servers (vite, cargo, npm) emit color codes that render as escape garbage in the log viewer and exported logs.
4. **Cap the frontend log buffer to match the backend's 5000-line limit** — the ServersTab appends batches unbounded, so a chatty server viewed for hours balloons the webview's memory.
5. **Sticky log auto-scroll** — new output currently always yanks the view to the bottom; only follow when the user is already at the bottom so scrolling back to read isn't fought.
6. **Surface server start/stop/restart failures in the UI** — errors (bad path, port in use) currently go to `console.error` only; show a dismissible error banner in the Servers tab.
7. **Fetch favicons from the site itself first** — try `{origin}/favicon.ico` with a content-type check and size cap before falling back to the Google favicon service (privacy + works for LAN/intranet links), and stop caching HTML error pages as icons.
8. **Bound and gate `fetch_url_metadata`** — the title fetch reads the entire body into memory with no size cap or content-type check; cap at 256 KB, require HTML, and decode HTML entities in extracted titles ("Foo &amp;amp; Bar").
9. **Prune stale monitor statuses in the check loop** — statuses of monitors removed via config import linger forever and keep counting in the status bar; retain only current monitor ids.
10. **Validate ping monitor targets** — the target string is passed straight to the `ping` CLI, so a target starting with `-` injects options; restrict to hostname/IP characters.

## New Features

1. **Pause/resume status monitors** — per-monitor `enabled` flag with a context-menu toggle; paused monitors are skipped by the check loop and shown with a hollow dot.
2. **Start/stop servers from the tray menu** — tray server entries become clickable (start if stopped, stop if running) and show crashed state distinctly.
3. **Clear server log action** — logs now persist across stop/crash, so add a `clear_server_log` command and a Clear button in the log viewer.
4. **Duplicate apps and servers from the context menu** — clone an entry as "Name (copy)" inserted next to the original (auto-launch off on server copies).
5. **Window geometry persistence** — remember size/position in settings on close/quit and restore them at startup.
