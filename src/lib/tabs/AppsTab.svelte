<script>
  import { invoke } from "@tauri-apps/api/core";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  let apps = $state([]);
  let showAddModal = $state(false);
  let editingApp = $state(null);
  let contextMenu = $state(null);

  // Form state
  let formMode = $state("simple"); // "simple" or "command"
  let formName = $state("");
  let formRawCommand = $state("");
  let formExecutable = $state("");
  let formArguments = $state("");
  let formWorkingDir = $state("");
  let formTypeLabel = $state("");
  let formIcon = $state("");
  let formEnvVars = $state([]);

  // Icon cache: app id -> base64 data URL
  let iconCache = $state({});

  onMount(loadApps);

  async function loadApps() {
    apps = await invoke("get_apps");
    loadIcons();
  }

  async function loadIcons() {
    for (const app of apps) {
      if (app.icon && !iconCache[app.id]) {
        try {
          const data = await invoke("read_icon_base64", { path: app.icon });
          iconCache[app.id] = data;
          iconCache = iconCache; // trigger reactivity
        } catch (_) {
          // Icon not found, will show fallback
        }
      }
    }
  }

  function openAddModal() {
    editingApp = null;
    resetForm();
    showAddModal = true;
  }

  function openEditModal(app) {
    editingApp = app;
    formName = app.name;
    formMode = app.raw_command ? "command" : "simple";
    formRawCommand = app.raw_command || "";
    formExecutable = app.executable || "";
    formArguments = app.arguments.join(" ");
    formWorkingDir = app.working_directory || "";
    formTypeLabel = app.type_label || "";
    formIcon = app.icon || "";
    formEnvVars = app.env_vars.length > 0
      ? app.env_vars.map(e => ({ ...e }))
      : [];
    showAddModal = true;
    contextMenu = null;
  }

  function resetForm() {
    formMode = "simple";
    formName = "";
    formRawCommand = "";
    formExecutable = "";
    formArguments = "";
    formWorkingDir = "";
    formTypeLabel = "";
    formIcon = "";
    formEnvVars = [];
  }

  function addEnvVar() {
    formEnvVars = [...formEnvVars, { key: "", value: "" }];
  }

  function removeEnvVar(index) {
    formEnvVars = formEnvVars.filter((_, i) => i !== index);
  }

  async function saveApp() {
    const args = formArguments.trim() ? formArguments.trim().split(/\s+/) : [];
    const envVars = formEnvVars.filter(e => e.key.trim() !== "");
    const params = {
      name: formName,
      rawCommand: formMode === "command" ? (formRawCommand || null) : null,
      executable: formMode === "simple" ? (formExecutable || null) : null,
      arguments: formMode === "simple" ? args : [],
      workingDirectory: formWorkingDir || null,
      envVars: formMode === "simple" ? envVars : [],
      icon: formIcon || null,
      typeLabel: formTypeLabel || null,
    };

    if (editingApp) {
      await invoke("update_app", { id: editingApp.id, ...params });
    } else {
      await invoke("add_app", params);
    }

    showAddModal = false;
    resetForm();
    await loadApps();
  }

  async function browseIcon() {
    const file = await openDialog({
      title: "Select icon image",
      filters: [{ name: "Images", extensions: ["png", "svg", "xpm", "ico"] }],
      multiple: false,
      directory: false,
    });
    if (file) {
      formIcon = file;
    }
  }

  async function importDesktopFile() {
    const file = await openDialog({
      title: "Import .desktop file",
      filters: [{ name: "Desktop Entry", extensions: ["desktop"] }],
      multiple: false,
      directory: false,
    });
    if (!file) return;
    try {
      await invoke("add_app_from_desktop", { path: file });
      await loadApps();
    } catch (e) {
      console.error("Import failed:", e);
    }
  }

  async function launchApp(app) {
    try {
      await invoke("launch_app", { id: app.id });
    } catch (e) {
      console.error("Launch failed:", e);
    }
  }

  function showContextMenu(event, app) {
    event.preventDefault();
    contextMenu = {
      x: event.clientX,
      y: event.clientY,
      app,
    };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  async function removeApp(app) {
    await invoke("remove_app", { id: app.id });
    contextMenu = null;
    await loadApps();
  }

  async function openDirectory(app) {
    try {
      await invoke("open_app_directory", { id: app.id });
    } catch (e) {
      console.error("Failed to open directory:", e);
    }
    contextMenu = null;
  }
</script>

<svelte:window onclick={closeContextMenu} />

<div class="tab-content">
  <div class="tab-header">
    <h2>Apps</h2>
    <div class="header-actions">
      <button class="add-btn" onclick={importDesktopFile}>Import .desktop</button>
      <button class="add-btn" onclick={openAddModal}>+ Add App</button>
    </div>
  </div>

  {#if apps.length === 0}
    <div class="empty-state">
      <p>No applications configured yet.</p>
      <p>Click "+ Add App" to add your first application.</p>
    </div>
  {:else}
    <div class="app-list">
      {#each apps as app}
        <button
          class="app-card"
          onclick={() => launchApp(app)}
          oncontextmenu={(e) => showContextMenu(e, app)}
        >
          <div class="app-icon">
            {#if iconCache[app.id]}
              <img src={iconCache[app.id]} alt="" />
            {:else}
              {app.name.charAt(0).toUpperCase()}
            {/if}
          </div>
          <div class="app-info">
            <span class="app-name">{app.name}</span>
            {#if app.type_label}
              <span class="app-type">{app.type_label}</span>
            {/if}
            <span class="app-path">{app.raw_command || app.executable || ""}</span>
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>

{#if contextMenu}
  <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
    <button onclick={() => openEditModal(contextMenu.app)}>Edit</button>
    <button onclick={() => openDirectory(contextMenu.app)}>Open Directory</button>
    <hr />
    <button class="danger" onclick={() => removeApp(contextMenu.app)}>Remove</button>
  </div>
{/if}

{#if showAddModal}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => showAddModal = false} onkeydown={() => {}}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="modal-header">
        <h2>{editingApp ? "Edit App" : "Add App"}</h2>
        <button class="close-btn" onclick={() => showAddModal = false}>&times;</button>
      </div>
      <form class="modal-body" onsubmit={(e) => { e.preventDefault(); saveApp(); }}>
        <label>
          Name
          <input type="text" bind:value={formName} required placeholder="My Application" />
        </label>

        <label>
          Type Label
          <input type="text" bind:value={formTypeLabel} placeholder="game, tool, etc." />
        </label>

        <label>
          Icon
          <div class="icon-input-row">
            <input type="text" bind:value={formIcon} placeholder="/path/to/icon.png" />
            <button type="button" class="browse-btn" onclick={browseIcon}>Browse</button>
          </div>
        </label>

        <div class="mode-toggle">
          <button type="button" class:active={formMode === "simple"} onclick={() => formMode = "simple"}>Simple</button>
          <button type="button" class:active={formMode === "command"} onclick={() => formMode = "command"}>Command</button>
        </div>

        {#if formMode === "command"}
          <label>
            Shell Command
            <textarea bind:value={formRawCommand} required placeholder='env WINEPREFIX="/home/user/.wine" wine-stable C:\\windows\\command\\start.exe ...' rows="3"></textarea>
            <span class="hint">Full command as you'd type it in a terminal. Executed via sh -c.</span>
          </label>
        {:else}
          <label>
            Executable Path
            <input type="text" bind:value={formExecutable} required placeholder="/usr/bin/app" />
          </label>

          <label>
            Arguments
            <input type="text" bind:value={formArguments} placeholder="--flag value" />
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

        <div class="modal-actions">
          <button type="button" class="cancel-btn" onclick={() => showAddModal = false}>Cancel</button>
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
  }

  .tab-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
  }

  .tab-header h2 {
    margin: 0;
    font-size: 1.2em;
  }

  .header-actions {
    display: flex;
    gap: 8px;
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
    height: 50%;
    color: var(--text-muted);
    font-size: 0.95em;
  }

  .empty-state p {
    margin: 4px 0;
  }

  .app-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .app-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
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

  .app-card:hover {
    background: var(--hover);
  }

  .app-icon {
    width: 40px;
    height: 40px;
    border-radius: 8px;
    background: var(--accent);
    color: white;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 600;
    font-size: 1.1em;
    flex-shrink: 0;
    overflow: hidden;
  }

  .app-icon img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .app-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .app-name {
    font-weight: 500;
  }

  .app-type {
    font-size: 0.8em;
    color: var(--text-muted);
    text-transform: capitalize;
  }

  .app-path {
    font-size: 0.8em;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  input[type="text"] {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.95rem;
    font-family: inherit;
  }

  input[type="text"]:focus,
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

  .hint {
    font-size: 0.8em;
    color: var(--text-muted);
    font-weight: 400;
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

  .icon-input-row {
    display: flex;
    gap: 6px;
  }

  .icon-input-row input {
    flex: 1;
  }

  .browse-btn {
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    font-size: 0.85em;
    font-family: inherit;
    white-space: nowrap;
  }

  .browse-btn:hover {
    background: var(--hover);
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
  }

  .save-btn {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    background: var(--accent);
    color: white;
    cursor: pointer;
    font-weight: 500;
  }

  .save-btn:hover {
    opacity: 0.9;
  }
</style>
