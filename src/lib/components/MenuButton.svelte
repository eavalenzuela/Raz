<script>
  import { invoke } from "@tauri-apps/api/core";

  let showMenu = $state(false);
  let settingsOpen = $state(false);
  let aboutOpen = $state(false);

  // Settings form state
  let defaultCheckInterval = $state(60);
  let notificationsEnabled = $state(true);
  let notifyOnDown = $state(true);
  let notifyOnUp = $state(true);
  let minimizeToTray = $state(true);

  function toggleMenu() {
    showMenu = !showMenu;
  }

  function closeMenu() {
    showMenu = false;
  }

  async function openSettings() {
    closeMenu();
    try {
      const s = await invoke("get_settings");
      defaultCheckInterval = s.default_check_interval_secs;
      notificationsEnabled = s.notifications_enabled;
      notifyOnDown = s.notify_on_down;
      notifyOnUp = s.notify_on_up;
      minimizeToTray = s.minimize_to_tray;
    } catch (_) {}
    settingsOpen = true;
  }

  async function saveSettings() {
    await invoke("update_settings", {
      settings: {
        default_check_interval_secs: defaultCheckInterval,
        notifications_enabled: notificationsEnabled,
        notify_on_down: notifyOnDown,
        notify_on_up: notifyOnUp,
        minimize_to_tray: minimizeToTray,
      },
    });
    settingsOpen = false;
  }

  async function createDesktopEntry() {
    closeMenu();
    try {
      const path = await invoke("create_desktop_entry");
      desktopMsg = `Created: ${path}`;
    } catch (e) {
      desktopMsg = `Error: ${e}`;
    }
    setTimeout(() => desktopMsg = null, 4000);
  }

  let desktopMsg = $state(null);

  async function quitApp() {
    await invoke("quit_app");
  }
</script>

<div class="menu-wrapper">
  <button class="menu-button" onclick={toggleMenu} aria-label="Menu">
    <svg width="18" height="18" viewBox="0 0 18 18" fill="currentColor">
      <rect y="2" width="18" height="2" rx="1" />
      <rect y="8" width="18" height="2" rx="1" />
      <rect y="14" width="18" height="2" rx="1" />
    </svg>
  </button>

  {#if showMenu}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="menu-backdrop" onclick={closeMenu} onkeydown={() => {}}></div>
    <div class="menu-dropdown">
      <button onclick={openSettings}>Settings</button>
      <button onclick={createDesktopEntry}>Create Desktop Icon</button>
      <button onclick={() => { aboutOpen = true; closeMenu(); }}>About</button>
      <hr />
      <button onclick={quitApp}>Quit</button>
    </div>
  {/if}
</div>

{#if desktopMsg}
  <div class="toast">{desktopMsg}</div>
{/if}

{#if aboutOpen}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => aboutOpen = false} onkeydown={() => {}}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal about-modal" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="modal-header">
        <h2>About Raz</h2>
        <button class="close-btn" onclick={() => aboutOpen = false}>&times;</button>
      </div>
      <div class="about-body">
        <div class="about-title">Raz</div>
        <div class="about-version">v1.0.0</div>
        <p class="about-description">A minimal personal launcher and homepage for launching apps, bookmarking links, and monitoring system and service status.</p>
        <div class="about-meta">
          <span>Built with Tauri v2 + Svelte</span>
        </div>
      </div>
    </div>
  </div>
{/if}

{#if settingsOpen}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => settingsOpen = false} onkeydown={() => {}}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="modal-header">
        <h2>Settings</h2>
        <button class="close-btn" onclick={() => settingsOpen = false}>&times;</button>
      </div>
      <form class="modal-body" onsubmit={(e) => { e.preventDefault(); saveSettings(); }}>
        <fieldset>
          <legend>Status Monitor</legend>
          <label>
            Default check interval (seconds)
            <input type="number" bind:value={defaultCheckInterval} min="10" step="10" />
          </label>
        </fieldset>

        <fieldset>
          <legend>Notifications</legend>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={notificationsEnabled} />
            Enable desktop notifications
          </label>
          <label class="checkbox-label sub" class:disabled={!notificationsEnabled}>
            <input type="checkbox" bind:checked={notifyOnDown} disabled={!notificationsEnabled} />
            Notify when a target goes down
          </label>
          <label class="checkbox-label sub" class:disabled={!notificationsEnabled}>
            <input type="checkbox" bind:checked={notifyOnUp} disabled={!notificationsEnabled} />
            Notify when a target comes back up
          </label>
        </fieldset>

        <fieldset>
          <legend>Window</legend>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={minimizeToTray} />
            Minimize to tray on close
          </label>
        </fieldset>

        <div class="modal-actions">
          <button type="button" class="cancel-btn" onclick={() => settingsOpen = false}>Cancel</button>
          <button type="submit" class="save-btn">Save</button>
        </div>
      </form>
    </div>
  </div>
{/if}

<style>
  .menu-wrapper {
    position: relative;
  }

  .menu-button {
    background: none;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
    cursor: pointer;
    color: var(--text);
    display: flex;
    align-items: center;
  }

  .menu-button:hover {
    background: var(--hover);
  }

  .menu-backdrop {
    position: fixed;
    inset: 0;
    z-index: 99;
  }

  .menu-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    margin-top: 4px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    z-index: 100;
    min-width: 150px;
    padding: 4px 0;
  }

  .menu-dropdown button {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    padding: 8px 16px;
    cursor: pointer;
    color: var(--text);
    font-size: 0.9em;
  }

  .menu-dropdown button:hover {
    background: var(--hover);
  }

  .menu-dropdown hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }

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
    gap: 16px;
  }

  fieldset {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  legend {
    font-size: 0.85em;
    font-weight: 500;
    color: var(--text-muted);
    padding: 0 4px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 0.85em;
    font-weight: 500;
    color: var(--text-muted);
  }

  .checkbox-label {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    color: var(--text);
    font-weight: 400;
  }

  .checkbox-label.sub {
    padding-left: 24px;
  }

  .checkbox-label.disabled {
    opacity: 0.5;
  }

  .checkbox-label input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  input[type="number"] {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.95rem;
    font-family: inherit;
    width: 120px;
  }

  input[type="number"]:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
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

  /* Toast */
  .toast {
    position: fixed;
    bottom: 40px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 8px 16px;
    font-size: 0.85em;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    z-index: 500;
    white-space: nowrap;
  }

  /* About modal */
  .about-modal {
    width: 360px;
  }

  .about-body {
    padding: 24px 20px;
    text-align: center;
  }

  .about-title {
    font-size: 1.4em;
    font-weight: 600;
  }

  .about-version {
    font-size: 0.85em;
    color: var(--text-muted);
    margin-top: 2px;
  }

  .about-description {
    font-size: 0.9em;
    color: var(--text);
    margin: 16px 0;
    line-height: 1.5;
  }

  .about-meta {
    font-size: 0.8em;
    color: var(--text-muted);
  }
</style>
