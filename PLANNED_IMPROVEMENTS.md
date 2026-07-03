# Planned Improvements & Features

## Improvements

1. **Stop servers by killing the whole process group** — `stop_server` only kills the direct child, so `bash -c` servers orphan the real process; spawn in a new process group and SIGTERM/SIGKILL the group.
2. **Persist server logs after exit** — logs currently vanish when a process stops/crashes because the buffer lives inside `RunningServer`; keep per-server log buffers in the manager so stopped/crashed servers can still be viewed and exported.
3. **Report crashed state distinctly + crash notifications** — the watcher silently drops crashed servers so the UI shows "stopped"; record exit status, surface "crashed" in the UI/tray, and send a desktop notification (respecting settings).
4. **Apply env vars in raw-command mode** — `launch_app`/`spawn_server` ignore configured env vars for `bash -c` commands and the form hides the editor; apply them in both backends and keep the editor visible in Command mode.
5. **HTTP monitors treat 4xx/5xx as down** — `check_target` counts any response (even 500) as "up"; only 2xx/3xx should count.
6. **Parallelize monitor checks** — checks run serially, so one down target with a 10s timeout delays every other monitor; run due checks on threads and join.
7. **Normalize scheme-less URLs on link add/update** — "example.com" currently breaks favicon fetch and open; prepend `https://` when no scheme is present.
8. **Use the configured default check interval in the Add Monitor form** — the Settings value `default_check_interval_secs` exists but the form hardcodes 60.
9. **Quit terminates managed servers** — both menu Quit and tray Quit call `std::process::exit(0)` and orphan every managed server; gracefully terminate them first.
10. **Fix drag-reorder during an active search filter** — dropping while filtered assigns the filtered subset back to `apps`, silently hiding all non-matching apps until reload; disable drag while a filter is active.

## New Features

1. **Command palette (Ctrl+K)** — fuzzy-find across apps, links, and servers with keyboard navigation; Enter launches/opens/starts.
2. **Config export/import** — menu actions to back up the whole config to JSON and restore it, with file dialogs and live reload of all tabs.
3. **Start at login** — Settings toggle that writes/removes `~/.config/autostart/raz.desktop`.
4. **Monitor uptime history** — keep the last 30 check results per monitor and render them as an uptime-bead strip in the sidebar.
5. **Restart server action** — one-click restart (row button + context menu) backed by a `restart_server` command that stops the group and respawns.
