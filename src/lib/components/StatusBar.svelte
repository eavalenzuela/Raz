<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";

  let monitors = $state([]);
  let statuses = $state([]);
  let unlisten = null;

  onMount(async () => {
    await refresh();
    unlisten = await listen("monitor-update", refresh);
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  async function refresh() {
    monitors = await invoke("get_status_monitors");
    statuses = await invoke("get_monitor_statuses");
  }

  function summary() {
    if (monitors.length === 0) return { text: "No monitors configured", state: "none" };
    const down = statuses.filter(s => s.state === "down").length;
    const total = monitors.length;
    const checked = statuses.length;
    if (checked === 0) return { text: "Checking...", state: "unknown" };
    if (down === 0) return { text: "All systems online", state: "online" };
    return { text: `${down} of ${total} targets down`, state: "offline" };
  }

  function lastCheck() {
    if (statuses.length === 0) return "No checks yet";
    const oldest = Math.max(...statuses.map(s => s.last_check ?? 0));
    if (oldest < 60) return `Last checked: ${oldest}s ago`;
    if (oldest < 3600) return `Last checked: ${Math.floor(oldest / 60)}m ago`;
    return `Last checked: ${Math.floor(oldest / 3600)}h ago`;
  }
</script>

<div class="status-bar">
  <span class="status-indicator {summary().state}"></span>
  <span>{summary().text}</span>
  <span class="spacer"></span>
  <span class="timestamp">{lastCheck()}</span>
</div>

<style>
  .status-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    border-top: 1px solid var(--border);
    font-size: 0.8em;
    color: var(--text-muted);
    background: var(--surface);
    flex-shrink: 0;
  }

  .status-indicator {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-indicator.online { background: #22c55e; }
  .status-indicator.offline { background: #ef4444; }
  .status-indicator.unknown { background: #a3a3a3; }
  .status-indicator.none { background: #a3a3a3; }

  .spacer { flex: 1; }
</style>
