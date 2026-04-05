<script>
  import { invoke } from "@tauri-apps/api/core";

  let showMenu = $state(false);
  let settingsOpen = $state(false);

  function toggleMenu() {
    showMenu = !showMenu;
  }

  function closeMenu() {
    showMenu = false;
  }

  function openSettings() {
    settingsOpen = true;
    closeMenu();
  }

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
      <button onclick={closeMenu}>About</button>
      <hr />
      <button onclick={quitApp}>Quit</button>
    </div>
  {/if}
</div>

{#if settingsOpen}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => settingsOpen = false} onkeydown={() => {}}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="modal-header">
        <h2>Settings</h2>
        <button class="close-btn" onclick={() => settingsOpen = false}>&times;</button>
      </div>
      <div class="modal-body">
        <p>Settings will go here.</p>
      </div>
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
  }
</style>
