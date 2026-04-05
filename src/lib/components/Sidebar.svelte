<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { sendNotification } from "@tauri-apps/plugin-notification";
  import { onMount, onDestroy } from "svelte";

  let collapsed = $state(false);
  let pinned = $state([]);
  let monitors = $state([]);
  let monitorStatuses = $state({});
  let showMonitorModal = $state(false);
  let editingMonitor = $state(null);
  let contextMenu = $state(null);

  // Monitor form
  let formName = $state("");
  let formTarget = $state("");
  let formCheckType = $state("http");
  let formInterval = $state(60);

  let unlistenUpdate = null;
  let unlistenNotify = null;
  let unlistenPinned = null;
  let pollInterval = null;

  onMount(async () => {
    await loadPinned();
    await loadMonitors();
    await pollMonitorStatuses();
    pollInterval = setInterval(pollMonitorStatuses, 5000);

    unlistenUpdate = await listen("monitor-update", () => {
      pollMonitorStatuses();
      loadMonitors();
    });

    unlistenNotify = await listen("monitor-notification", (event) => {
      sendNotification({ title: "Raz", body: event.payload });
    });

    unlistenPinned = await listen("pinned-changed", () => {
      loadPinned();
    });
  });

  onDestroy(() => {
    if (unlistenUpdate) unlistenUpdate();
    if (unlistenNotify) unlistenNotify();
    if (unlistenPinned) unlistenPinned();
    if (pollInterval) clearInterval(pollInterval);
  });

  async function loadPinned() {
    pinned = await invoke("get_pinned");
  }

  async function loadMonitors() {
    monitors = await invoke("get_status_monitors");
  }

  async function pollMonitorStatuses() {
    const list = await invoke("get_monitor_statuses");
    const map = {};
    for (const s of list) {
      map[s.id] = s;
    }
    monitorStatuses = map;
  }

  async function unpinItem(item) {
    await invoke("unpin_item", { id: item.id });
    await loadPinned();
  }

  async function activatePin(item) {
    if (item.source_type === "app") {
      await invoke("launch_app", { id: item.source_id });
    } else if (item.source_type === "link") {
      // Find the link URL
      const links = await invoke("get_links");
      const link = links.find(l => l.id === item.source_id);
      if (link) {
        await invoke("open_link", { url: link.url });
      }
    }
  }

  function monitorState(monitor) {
    const s = monitorStatuses[monitor.id];
    return s ? s.state : "unknown";
  }

  function lastCheckLabel(monitor) {
    const s = monitorStatuses[monitor.id];
    if (!s || s.last_check === null) return "";
    const secs = s.last_check;
    if (secs < 60) return `${secs}s ago`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ago`;
    return `${Math.floor(secs / 3600)}h ago`;
  }

  function openAddMonitor() {
    editingMonitor = null;
    formName = "";
    formTarget = "";
    formCheckType = "http";
    formInterval = 60;
    showMonitorModal = true;
  }

  function openEditMonitor(monitor) {
    editingMonitor = monitor;
    formName = monitor.name;
    formTarget = monitor.target;
    formCheckType = monitor.check_type;
    formInterval = monitor.check_interval_secs;
    showMonitorModal = true;
    contextMenu = null;
  }

  async function saveMonitor() {
    const params = {
      name: formName,
      target: formTarget,
      checkType: formCheckType,
      checkIntervalSecs: formInterval,
    };
    if (editingMonitor) {
      await invoke("update_status_monitor", { id: editingMonitor.id, ...params });
    } else {
      await invoke("add_status_monitor", params);
    }
    showMonitorModal = false;
    await loadMonitors();
  }

  function showContextMenu(event, monitor) {
    event.preventDefault();
    event.stopPropagation();
    contextMenu = { x: event.clientX, y: event.clientY, monitor };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  async function removeMonitor(monitor) {
    await invoke("remove_status_monitor", { id: monitor.id });
    contextMenu = null;
    await loadMonitors();
  }
</script>

<svelte:window onclick={closeContextMenu} />

<div class="sidebar" class:collapsed>
  <button class="collapse-toggle" onclick={() => collapsed = !collapsed} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}>
    {collapsed ? "\u25C0" : "\u25B6"}
  </button>

  {#if !collapsed}
    <div class="sidebar-content">
      <section class="sidebar-section">
        <h3>Pinned</h3>
        {#if pinned.length === 0}
          <div class="placeholder">No pinned items</div>
        {:else}
          <div class="pinned-list">
            {#each pinned as item}
              <div class="pinned-item">
                <button class="pinned-btn" onclick={() => activatePin(item)}>
                  <span class="pin-type">{item.source_type === "app" ? "\u25B6" : "\u{1F517}"}</span>
                  <span class="pin-name">{item.name}</span>
                </button>
                <button class="unpin-btn" onclick={() => unpinItem(item)} title="Unpin">&times;</button>
              </div>
            {/each}
          </div>
        {/if}
      </section>

      <hr />

      <section class="sidebar-section">
        <div class="section-header">
          <h3>Status Monitor</h3>
          <button class="add-monitor-btn" onclick={openAddMonitor} title="Add monitor">+</button>
        </div>
        {#if monitors.length === 0}
          <div class="placeholder">No monitors configured</div>
        {:else}
          <div class="monitor-list">
            {#each monitors as monitor}
              <div
                class="monitor-item"
                oncontextmenu={(e) => showContextMenu(e, monitor)}
                role="listitem"
              >
                <span class="monitor-dot {monitorState(monitor)}"></span>
                <span class="monitor-name">{monitor.name}</span>
                <span class="monitor-time">{lastCheckLabel(monitor)}</span>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    </div>
  {/if}
</div>

{#if contextMenu}
  <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
    <button onclick={() => openEditMonitor(contextMenu.monitor)}>Edit</button>
    <hr />
    <button class="danger" onclick={() => removeMonitor(contextMenu.monitor)}>Remove</button>
  </div>
{/if}

{#if showMonitorModal}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => showMonitorModal = false} onkeydown={() => {}}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="modal-header">
        <h2>{editingMonitor ? "Edit Monitor" : "Add Monitor"}</h2>
        <button class="close-btn" onclick={() => showMonitorModal = false}>&times;</button>
      </div>
      <form class="modal-body" onsubmit={(e) => { e.preventDefault(); saveMonitor(); }}>
        <label>
          Name
          <input type="text" bind:value={formName} required placeholder="My Server" />
        </label>

        <label>
          Target
          <input type="text" bind:value={formTarget} required placeholder={formCheckType === "http" ? "https://example.com" : "192.168.1.1"} />
        </label>

        <label>
          Check Type
          <select bind:value={formCheckType}>
            <option value="http">HTTP</option>
            <option value="ping">Ping</option>
          </select>
        </label>

        <label>
          Check Interval (seconds)
          <input type="number" bind:value={formInterval} min="10" step="10" />
        </label>

        <div class="modal-actions">
          <button type="button" class="cancel-btn" onclick={() => showMonitorModal = false}>Cancel</button>
          <button type="submit" class="save-btn">Save</button>
        </div>
      </form>
    </div>
  </div>
{/if}

<style>
  .sidebar {
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    min-width: 220px;
    width: 220px;
    transition: min-width 0.2s, width 0.2s;
    position: relative;
  }

  .sidebar.collapsed {
    min-width: 32px;
    width: 32px;
  }

  .collapse-toggle {
    position: absolute;
    top: 8px;
    left: -12px;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--surface);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.6em;
    color: var(--text-muted);
    z-index: 10;
    padding: 0;
  }

  .collapse-toggle:hover {
    background: var(--hover);
  }

  .sidebar-content {
    padding: 12px;
    overflow-y: auto;
    flex: 1;
  }

  .sidebar-section h3 {
    font-size: 0.8em;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    margin: 0 0 8px 0;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 8px;
  }

  .section-header h3 {
    margin: 0;
  }

  .add-monitor-btn {
    width: 22px;
    height: 22px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface);
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.9em;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
  }

  .add-monitor-btn:hover {
    background: var(--hover);
    color: var(--text);
  }

  .placeholder {
    font-size: 0.85em;
    color: var(--text-muted);
    font-style: italic;
  }

  hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 12px 0;
  }

  /* Pinned items */
  .pinned-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .pinned-item {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .pinned-btn {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border: none;
    border-radius: 4px;
    background: none;
    cursor: pointer;
    color: var(--text);
    font-size: 0.85em;
    font-family: inherit;
    text-align: left;
    min-width: 0;
  }

  .pinned-btn:hover {
    background: var(--hover);
  }

  .pin-type {
    flex-shrink: 0;
    font-size: 0.8em;
  }

  .pin-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unpin-btn {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 1em;
    padding: 2px 4px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .pinned-item:hover .unpin-btn {
    opacity: 1;
  }

  .unpin-btn:hover {
    color: #ef4444;
  }

  /* Monitor items */
  .monitor-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .monitor-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 0.85em;
    cursor: default;
  }

  .monitor-item:hover {
    background: var(--hover);
  }

  .monitor-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .monitor-dot.up { background: #22c55e; }
  .monitor-dot.down { background: #ef4444; }
  .monitor-dot.unknown { background: #a3a3a3; }

  .monitor-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .monitor-time {
    font-size: 0.8em;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  /* Context menu */
  .context-menu {
    position: fixed;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    z-index: 300;
    min-width: 140px;
    padding: 4px 0;
  }

  .context-menu button {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    padding: 8px 16px;
    cursor: pointer;
    color: var(--text);
    font-size: 0.9em;
    font-family: inherit;
  }

  .context-menu button:hover { background: var(--hover); }
  .context-menu button.danger { color: #ef4444; }
  .context-menu hr { border: none; border-top: 1px solid var(--border); margin: 4px 0; }

  /* Modal */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }

  .modal {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: 400px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 { margin: 0; font-size: 1.1em; }

  .close-btn {
    background: none;
    border: none;
    font-size: 1.5em;
    cursor: pointer;
    color: var(--text);
    padding: 0 4px;
  }

  .modal-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.85em;
    font-weight: 500;
    color: var(--text-muted);
  }

  input[type="text"],
  input[type="number"],
  select {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.95rem;
    font-family: inherit;
  }

  input:focus, select:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding-top: 8px;
  }

  .cancel-btn {
    padding: 8px 16px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    font-family: inherit;
  }

  .save-btn {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    background: var(--accent);
    color: white;
    cursor: pointer;
    font-weight: 500;
    font-family: inherit;
  }

  .save-btn:hover { opacity: 0.9; }
</style>
