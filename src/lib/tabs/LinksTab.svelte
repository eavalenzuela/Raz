<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let links = $state([]);
  let showModal = $state(false);
  let editingLink = $state(null);
  let contextMenu = $state(null);

  // Form state
  let formName = $state("");
  let formUrl = $state("");
  let formIcon = $state("");

  onMount(loadLinks);

  async function loadLinks() {
    links = await invoke("get_links");
  }

  function openAddModal() {
    editingLink = null;
    resetForm();
    showModal = true;
  }

  function openEditModal(link) {
    editingLink = link;
    formName = link.name;
    formUrl = link.url;
    formIcon = link.icon || "";
    showModal = true;
    contextMenu = null;
  }

  function resetForm() {
    formName = "";
    formUrl = "";
    formIcon = "";
  }

  async function saveLink() {
    const icon = formIcon || null;
    if (editingLink) {
      await invoke("update_link", { id: editingLink.id, name: formName, url: formUrl, icon });
    } else {
      await invoke("add_link", { name: formName, url: formUrl, icon });
    }
    showModal = false;
    resetForm();
    await loadLinks();
  }

  async function openLink(link) {
    try {
      await invoke("open_link", { url: link.url });
    } catch (e) {
      console.error("Failed to open link:", e);
    }
  }

  function showContextMenu(event, link) {
    event.preventDefault();
    contextMenu = { x: event.clientX, y: event.clientY, link };
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  async function pinLink(link) {
    try {
      await invoke("pin_item", { sourceId: link.id, sourceType: "link", name: link.name });
    } catch (e) {
      console.error("Pin failed:", e);
    }
    contextMenu = null;
  }

  async function removeLink(link) {
    await invoke("remove_link", { id: link.id });
    contextMenu = null;
    await loadLinks();
  }

  function faviconUrl(url) {
    try {
      const u = new URL(url);
      return `https://www.google.com/s2/favicons?domain=${u.hostname}&sz=64`;
    } catch {
      return null;
    }
  }

  function domainFromUrl(url) {
    try {
      return new URL(url).hostname;
    } catch {
      return url;
    }
  }
</script>

<svelte:window onclick={closeContextMenu} />

<div class="tab-content">
  <div class="tab-header">
    <h2>Links</h2>
    <button class="add-btn" onclick={openAddModal}>+ Add Link</button>
  </div>

  {#if links.length === 0}
    <div class="empty-state">
      <p>No bookmarks yet.</p>
      <p>Click "+ Add Link" to add your first bookmark.</p>
    </div>
  {:else}
    <div class="link-grid">
      {#each links as link}
        <button
          class="link-card"
          onclick={() => openLink(link)}
          oncontextmenu={(e) => showContextMenu(e, link)}
        >
          <div class="link-icon">
            {#if link.icon}
              <img src={link.icon} alt="" onerror={(e) => e.target.style.display='none'} />
            {:else}
              {#if faviconUrl(link.url)}
                <img src={faviconUrl(link.url)} alt="" onerror={(e) => e.target.style.display='none'} />
              {/if}
            {/if}
          </div>
          <span class="link-name">{link.name}</span>
          <span class="link-domain">{domainFromUrl(link.url)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

{#if contextMenu}
  <div class="context-menu" style="left: {contextMenu.x}px; top: {contextMenu.y}px;">
    <button onclick={() => openEditModal(contextMenu.link)}>Edit</button>
    <button onclick={() => pinLink(contextMenu.link)}>Pin to Sidebar</button>
    <hr />
    <button class="danger" onclick={() => removeLink(contextMenu.link)}>Remove</button>
  </div>
{/if}

{#if showModal}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="modal-backdrop" onclick={() => showModal = false} onkeydown={() => {}}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="modal" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="modal-header">
        <h2>{editingLink ? "Edit Link" : "Add Link"}</h2>
        <button class="close-btn" onclick={() => showModal = false}>&times;</button>
      </div>
      <form class="modal-body" onsubmit={(e) => { e.preventDefault(); saveLink(); }}>
        <label>
          Name
          <input type="text" bind:value={formName} required placeholder="Google" />
        </label>

        <label>
          URL
          <input type="text" bind:value={formUrl} required placeholder="https://google.com" />
        </label>

        <label>
          Icon URL <span class="hint">(optional — auto-fetches favicon if empty)</span>
          <input type="text" bind:value={formIcon} placeholder="https://example.com/icon.png" />
        </label>

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

  .link-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 12px;
  }

  .link-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 16px 12px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
    cursor: pointer;
    text-align: center;
    color: var(--text);
    font-family: inherit;
    font-size: inherit;
    width: 100%;
  }

  .link-card:hover {
    background: var(--hover);
  }

  .link-icon {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .link-icon img {
    width: 32px;
    height: 32px;
    object-fit: contain;
  }

  .link-name {
    font-weight: 500;
    font-size: 0.9em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }

  .link-domain {
    font-size: 0.75em;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
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
    width: 450px;
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

  input[type="text"] {
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-size: 0.95rem;
    font-family: inherit;
  }

  input[type="text"]:focus {
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

  .save-btn:hover {
    opacity: 0.9;
  }
</style>
