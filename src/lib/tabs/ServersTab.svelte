<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { save } from "@tauri-apps/plugin-dialog";
  import { onMount, onDestroy } from "svelte";

  let servers = $state([]);
  let statuses = $state({});
  let showModal = $state(false);
  let editingServer = $state(null);
  let contextMenu = $state(null);
  let selectedServerId = $state(null);
  let outputLines = $state([]);
  let outputEl = $state(null);
  let unlisten = null;
  let pollInterval = null;

  // Log search/filter
  let logSearch = $state("");
  let logLevelFilter = $state("all"); // "all", "stdout", "stderr"

  let filteredLines = $derived(() => {
    let lines = outputLines;
    if (logLevelFilter === "stderr") {
      lines = lines.filter(l => l.includes("[stderr]"));
    } else if (logLevelFilter === "stdout") {
      lines = lines.filter(l => !l.includes("[stderr]"));
    }
    if (logSearch.trim()) {
      const q = logSearch.toLowerCase();
      lines = lines.filter(l => l.toLowerCase().includes(q));
    }
    return lines;
  });

  // Resource monitoring
  let resources = $state(null);
  let resourceInterval = null;

  // Form state
  let formMode = $state("simple");
  let formName = $state("");
  let formRawCommand = $state("");
  let formExecutable = $state("");
  let formArguments = $state("");
  let formWorkingDir = $state("");
  let formAutoLaunch = $state(false);
  let formAutoRestart = $state(false);
  let formMaxRetries = $state(3);
  let formRestartCooldown = $state(5);
  let formEnvVars = $state([]);

  onMount(async () => {
    await loadServers();
    await pollStatuses();
    pollInterval = setInterval(pollStatuses, 3000);

    unlisten = await listen("server-output", (event) => {
      const [id, line] = event.payload;
      if (id === selectedServerId) {
        outputLines = [...outputLines, line];
        requestAnimationFrame(() => {
          if (outputEl) outputEl.scrollTop = outputEl.scrollHeight;
        });
      }
    });
  });

  onDestroy(() => {
    if (unlisten) unlisten();
    if (pollInterval) clearInterval(pollInterval);
    if (resourceInterval) clearInterval(resourceInterval);
  });

  async function loadServers() {
    servers = await invoke("get_servers");
  }

  async function pollStatuses() {
    const list = await invoke("get_all_server_statuses");
    const map = {};
    for (const s of list) {
      map[s.id] = s.state;
    }
    statuses = map;
  }

  function serverState(server) {
    return statuses[server.id] || "stopped";
  }

  function openAddModal() {
    editingServer = null;
    resetForm();
    showModal = true;
  }

  function openEditModal(server) {
    editingServer = server;
    formName = server.name;
    formMode = server.raw_command ? "command" : "simple";
    formRawCommand = server.raw_command || "";
    formExecutable = server.executable || "";
    formArguments = server.arguments.join(" ");
    formWorkingDir = server.working_directory || "";
    formAutoLaunch = server.auto_launch;
    formAutoRestart = server.auto_restart;
    formMaxRetries = server.max_retries;
    formRestartCooldown = server.restart_cooldown_secs;
    formEnvVars = server.env_vars.length > 0
      ? server.env_vars.map(e => ({ ...e }))
      : [];
    showModal = true;
    contextMenu = null;
  }

  function resetForm() {
    formMode = "simple";
    formName = "";
    formRawCommand = "";
    formExecutable = "";
    formArguments = "";
    formWorkingDir = "";
    formAutoLaunch = false;
    formAutoRestart = false;
    formMaxRetries = 3;
    formRestartCooldown = 5;
    formEnvVars = [];
  }

  function addEnvVar() {
    formEnvVars = [...formEnvVars, { key: "", value: "" }];
  }

  function removeEnvVar(index) {
    formEnvVars = formEnvVars.filter((_, i) => i !== index);
  }

  async function saveServer() {
    const args = formArguments.trim() ? formArguments.trim().split(/\s+/) : [];
    const envVars = formEnvVars.filter(e => e.key.trim() !== "");
    const params = {
      name: formName,
      rawCommand: formMode === "command" ? (formRawCommand || null) : null,
      executable: formMode === "simple" ? (formExecutable || null) : null,
      arguments: formMode === "simple" ? args : [],
      workingDirectory: formWorkingDir || null,
      envVars: formMode === "simple" ? envVars : [],
      autoLaunch: formAutoLaunch,
      autoRestart: formAutoRestart,
      maxRetries: formMaxRetries,
      restartCooldownSecs: formRestartCooldown,
    };

    if (editingServer) {
      await invoke("update_server", { id: editingServer.id, ...params });
    } else {
      await invoke("add_server", params);
    }

    showModal = false;
    resetForm();
    await loadServers();
  }

  async function startServer(server) {
    try {
      await invoke("start_server", { id: server.id });
      await pollStatuses();
    } catch (e) {
      console.error("Failed to start server:", e);
    }
  }

  async function stopServer(server) {
    try {
      await invoke("stop_server", { id: server.id });
      await pollStatuses();
      resources = null;
    } catch (e) {
      console.error("Failed to stop server:", e);
    }
  }

  async function selectServer(server) {
    selectedServerId = server.id;
    outputLines = await invoke("get_server_output", { id: server.id });
    logSearch = "";
    logLevelFilter = "all";
    requestAnimationFrame(() => {
      if (outputEl) outputEl.scrollTop = outputEl.scrollHeight;
    });

    // Start resource polling for this server
    if (resourceInterval) clearInterval(resourceInterval);
    resources = null;
    await pollResources();
    resourceInterval = setInterval(pollResources, 5000);
  }

  async function pollResources() {
    if (!selectedServerId) return;
    try {
      resources = await invoke("get_server_resources", { id: selectedServerId });
    } catch {
      resources = null;
    }
  }

  function showContextMenu(event, server) {
    event.preventDefault();
    contextMenu = { x: event.clientX, y: event.clientY, server };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  async function removeServer(server) {
    await invoke("remove_server", { id: server.id });
    contextMenu = null;
    if (selectedServerId === server.id) {
      selectedServerId = null;
      outputLines = [];
      resources = null;
      if (resourceInterval) clearInterval(resourceInterval);
    }
    await loadServers();
  }

  async function openDirectory(server) {
    try {
      await invoke("open_server_directory", { id: server.id });
    } catch (e) {
      console.error("Failed to open directory:", e);
    }
    contextMenu = null;
  }

  function viewLogs(server) {
    selectServer(server);
    contextMenu = null;
  }

  async function exportLog() {
    if (!selectedServerId) return;
    const path = await save({
      defaultPath: `server-log-${selectedServerId}.txt`,
      filters: [{ name: "Text", extensions: ["txt", "log"] }],
    });
    if (path) {
      try {
        await invoke("export_server_log", { id: selectedServerId, path });
      } catch (e) {
        console.error("Failed to export log:", e);
      }
    }
  }

  function formatUptime(secs) {
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    return `${h}h ${m}m`;
  }

  function formatMemory(kb) {
    if (kb < 1024) return `${kb} KB`;
    if (kb < 1024 * 1024) return `${(kb / 1024).toFixed(1)} MB`;
    return `${(kb / (1024 * 1024)).toFixed(2)} GB`;
  }
</script>

<svelte:window onclick={closeContextMenu} />

<div class="tab-content">
  <div class="tab-header">
    <h2>Servers</h2>
    <button class="add-btn" onclick={openAddModal}>+ Add Server</button>
  </div>

  {#if servers.length === 0}
    <div class="empty-state">
      <p>No local servers configured yet.</p>
      <p>Click "+ Add Server" to add your first server.</p>
    </div>
  {:else}
    <div class="servers-layout">
      <div class="server-list">
        {#each servers as server}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="server-row"
            class:selected={selectedServerId === server.id}
            onclick={() => selectServer(server)}
            oncontextmenu={(e) => showContextMenu(e, server)}
            onkeydown={() => {}}
            role="button"
            tabindex="0"
          >
            <span class="state-dot {serverState(server)}"></span>
            <span class="server-name">{server.name}</span>
            <span class="server-state-label">{serverState(server)}</span>
            {#if server.auto_launch}
              <span class="auto-badge">auto</span>
            {/if}
            {#if server.auto_restart}
              <span class="restart-badge">restart</span>
            {/if}
            <div class="server-actions">
              {#if serverState(server) === "running"}
                <button class="action-btn stop" onclick={(e) => { e.stopPropagation(); stopServer(server); }} title="Stop">&#9632;</button>
              {:else}
                <button class="action-btn start" onclick={(e) => { e.stopPropagation(); startServer(server); }} title="Start">&#9654;</button>
              {/if}
            </div>
          </div>
        {/each}
      </div>

      <div class="output-panel">
        {#if selectedServerId}
          <div class="output-header">
            <span class="output-title">Output: {servers.find(s => s.id === selectedServerId)?.name || ""}</span>
            <div class="output-controls">
              <select bind:value={logLevelFilter} class="level-filter">
                <option value="all">All</option>
                <option value="stdout">stdout</option>
                <option value="stderr">stderr</option>
              </select>
              <input
                type="text"
                class="log-search"
                placeholder="Search logs..."
                bind:value={logSearch}
              />
              <button class="export-btn" onclick={exportLog} title="Export log">Export</button>
            </div>
          </div>

          {#if resources}
            <div class="resource-bar">
              <span class="resource-item" title="Process ID">PID: {resources.pid}</span>
              <span class="resource-item" title="Uptime">Up: {formatUptime(resources.uptime_secs)}</span>
              <span class="resource-item" title="Memory (RSS)">Mem: {formatMemory(resources.memory_kb)}</span>
              <span class="resource-item" title="CPU Usage">CPU: {resources.cpu_percent.toFixed(1)}%</span>
            </div>
          {/if}

          <div class="output-log" bind:this={outputEl}>
            {#if filteredLines().length === 0}
              <span class="output-empty">{outputLines.length === 0 ? "No output yet." : "No matching lines."}</span>
            {:else}
              {#each filteredLines() as line}
                <div class="log-line" class:stderr={line.includes("[stderr]")}>{line}</div>
              {/each}
            {/if}
          </div>
        {:else}
          <div class="output-placeholder">
            <p>Select a server to view its output.</p>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

{#if contextMenu}
  <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
    <button onclick={() => openEditModal(contextMenu.server)}>Edit</button>
    <button onclick={() => openDirectory(contextMenu.server)}>Open Directory</button>
    <button onclick={() => viewLogs(contextMenu.server)}>View Logs</button>
    <hr />
    <button class="danger" onclick={() => removeServer(contextMenu.server)}>Remove</button>
  </div>
{/if}

{#if showModal}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => showModal = false} onkeydown={() => {}}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="modal-header">
        <h2>{editingServer ? "Edit Server" : "Add Server"}</h2>
        <button class="close-btn" onclick={() => showModal = false}>&times;</button>
      </div>
      <form class="modal-body" onsubmit={(e) => { e.preventDefault(); saveServer(); }}>
        <label>
          Name
          <input type="text" bind:value={formName} required placeholder="My Server" />
        </label>

        <div class="mode-toggle">
          <button type="button" class:active={formMode === "simple"} onclick={() => formMode = "simple"}>Simple</button>
          <button type="button" class:active={formMode === "command"} onclick={() => formMode = "command"}>Command</button>
        </div>

        {#if formMode === "command"}
          <label>
            Shell Command
            <textarea bind:value={formRawCommand} required placeholder="python3 -m http.server 8080" rows="3"></textarea>
            <span class="hint">Full command as you'd type it in a terminal.</span>
          </label>
        {:else}
          <label>
            Executable Path
            <input type="text" bind:value={formExecutable} required placeholder="/usr/bin/python3" />
          </label>

          <label>
            Arguments
            <input type="text" bind:value={formArguments} placeholder="-m http.server 8080" />
          </label>

          <fieldset>
            <legend>Environment Variables</legend>
            {#each formEnvVars as envVar, i}
              <div class="env-row">
                <input type="text" bind:value={envVar.key} placeholder="KEY" />
                <span>=</span>
                <input type="text" bind:value={envVar.value} placeholder="value" />
                <button type="button" class="remove-env" onclick={() => removeEnvVar(i)}>&times;</button>
              </div>
            {/each}
            <button type="button" class="add-env-btn" onclick={addEnvVar}>+ Add Variable</button>
          </fieldset>
        {/if}

        <label>
          Working Directory
          <input type="text" bind:value={formWorkingDir} placeholder="/home/user/project (optional)" />
        </label>

        <label class="checkbox-label">
          <input type="checkbox" bind:checked={formAutoLaunch} />
          Auto-launch when Raz starts
        </label>

        <fieldset class="restart-fieldset">
          <legend>Auto-Restart</legend>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={formAutoRestart} />
            Restart automatically if process exits
          </label>
          {#if formAutoRestart}
            <div class="restart-options">
              <label>
                Max retries
                <input type="number" bind:value={formMaxRetries} min="1" max="100" />
              </label>
              <label>
                Cooldown (seconds)
                <input type="number" bind:value={formRestartCooldown} min="0" max="300" />
              </label>
            </div>
          {/if}
        </fieldset>

        <div class="modal-actions">
          <button type="button" class="cancel-btn" onclick={() => showModal = false}>Cancel</button>
          <button type="submit" class="save-btn">Save</button>
        </div>
      </form>
    </div>
  </div>
{/if}

<style>
  .tab-content {
    padding: 16px;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  .tab-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
    flex-shrink: 0;
  }

  .tab-header h2 {
    margin: 0;
    font-size: 1.2em;
  }

  .add-btn {
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    font-size: 0.85em;
    font-family: inherit;
  }

  .add-btn:hover {
    background: var(--hover);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    color: var(--text-muted);
    font-size: 0.95em;
  }

  .empty-state p {
    margin: 4px 0;
  }

  .servers-layout {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    gap: 8px;
  }

  .server-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex-shrink: 0;
    max-height: 40%;
    overflow-y: auto;
  }

  .server-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    cursor: pointer;
    text-align: left;
    width: 100%;
    color: var(--text);
    font-family: inherit;
    font-size: inherit;
  }

  .server-row:hover {
    background: var(--hover);
  }

  .server-row.selected {
    border-color: var(--accent);
    background: var(--hover);
  }

  .state-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .state-dot.running { background: #22c55e; }
  .state-dot.stopped { background: #a3a3a3; }
  .state-dot.crashed { background: #ef4444; }

  .server-name {
    font-weight: 500;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .server-state-label {
    font-size: 0.8em;
    color: var(--text-muted);
    text-transform: capitalize;
  }

  .auto-badge, .restart-badge {
    font-size: 0.7em;
    background: var(--accent);
    color: white;
    padding: 1px 6px;
    border-radius: 4px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .restart-badge {
    background: #8b5cf6;
  }

  .server-actions {
    display: flex;
    gap: 4px;
  }

  .action-btn {
    width: 28px;
    height: 28px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.8em;
    color: var(--text);
    padding: 0;
  }

  .action-btn:hover {
    background: var(--hover);
  }

  .action-btn.start { color: #22c55e; }
  .action-btn.stop { color: #ef4444; }

  /* Output panel */
  .output-panel {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }

  .output-header {
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    font-size: 0.85em;
    font-weight: 500;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
  }

  .output-title {
    white-space: nowrap;
  }

  .output-controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .level-filter {
    padding: 3px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.85em;
    font-family: inherit;
  }

  .log-search {
    padding: 3px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.85em;
    font-family: inherit;
    width: 140px;
  }

  .log-search:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  .export-btn {
    padding: 3px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    font-size: 0.85em;
    font-family: inherit;
  }

  .export-btn:hover {
    background: var(--hover);
  }

  /* Resource bar */
  .resource-bar {
    padding: 4px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
    display: flex;
    gap: 16px;
    font-size: 0.78em;
    color: var(--text-muted);
    flex-shrink: 0;
    font-family: "Fira Code", "Cascadia Code", "Consolas", monospace;
  }

  .resource-item {
    white-space: nowrap;
  }

  .output-log {
    flex: 1;
    overflow-y: auto;
    padding: 8px 12px;
    background: #1a1a1a;
    font-family: "Fira Code", "Cascadia Code", "Consolas", monospace;
    font-size: 0.8em;
    line-height: 1.5;
    color: #d4d4d4;
  }

  .log-line {
    white-space: pre-wrap;
    word-break: break-all;
  }

  .log-line.stderr {
    color: #f87171;
  }

  .output-empty {
    color: #666;
    font-style: italic;
  }

  .output-placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 0.9em;
  }

  /* Context menu */
  .context-menu {
    position: fixed;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    z-index: 300;
    min-width: 160px;
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

  .context-menu button:hover {
    background: var(--hover);
  }

  .context-menu button.danger {
    color: #ef4444;
  }

  .context-menu hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }

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
    width: 500px;
    max-height: 80vh;
    overflow-y: auto;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
  }

  .modal-header h2 {
    margin: 0;
    font-size: 1.1em;
  }

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

  .hint {
    font-weight: 400;
  }

  input[type="text"],
  input[type="number"] {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.95rem;
    font-family: inherit;
  }

  input[type="text"]:focus,
  input[type="number"]:focus,
  textarea:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  textarea {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.95rem;
    font-family: inherit;
    resize: vertical;
  }

  .checkbox-label {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }

  .checkbox-label input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  .mode-toggle {
    display: flex;
    gap: 0;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
    width: fit-content;
  }

  .mode-toggle button {
    padding: 6px 16px;
    border: none;
    background: var(--surface);
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.85em;
    font-family: inherit;
  }

  .mode-toggle button.active {
    background: var(--accent);
    color: white;
  }

  fieldset {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px;
    margin: 0;
  }

  legend {
    font-size: 0.85em;
    font-weight: 500;
    color: var(--text-muted);
    padding: 0 4px;
  }

  .restart-fieldset {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .restart-options {
    display: flex;
    gap: 12px;
  }

  .restart-options label {
    flex: 1;
  }

  .restart-options input[type="number"] {
    width: 100%;
  }

  .env-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }

  .env-row input {
    flex: 1;
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.85rem;
    font-family: inherit;
  }

  .env-row span {
    color: var(--text-muted);
  }

  .remove-env {
    background: none;
    border: none;
    color: #ef4444;
    cursor: pointer;
    font-size: 1.2em;
    padding: 0 4px;
  }

  .add-env-btn {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 0.85em;
    padding: 4px 0;
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

  .save-btn:hover {
    opacity: 0.9;
  }
</style>
