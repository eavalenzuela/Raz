<script>
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let links = $state([]);
  let showModal = $state(false);
  let editingLink = $state(null);
  let contextMenu = $state(null);
  let collapsedFolders = $state(new Set());

  // Icon cache: link id -> local base64 data URL
  let iconCache = $state({});

  // Form state
  let formName = $state("");
  let formUrl = $state("");
  let formIcon = $state("");
  let formFolder = $state("");
  let fetchingTitle = $state(false);

  onMount(loadLinks);

  async function loadLinks() {
    links = await invoke("get_links");
    cacheFavicons();
  }

  async function cacheFavicons() {
    for (const link of links) {
      if (iconCache[link.id]) continue;
      if (link.icon) {
        // If icon is a local file path, load as base64
        if (link.icon.startsWith("/")) {
          try {
            const data = await invoke("read_icon_base64", { path: link.icon });
            iconCache[link.id] = data;
            iconCache = iconCache;
          } catch (_) {}
        } else {
          // External URL, use directly
          iconCache[link.id] = link.icon;
        }
      } else {
        // Fetch and cache favicon via backend
        try {
          const path = await invoke("fetch_favicon", { url: link.url });
          const data = await invoke("read_icon_base64", { path });
          iconCache[link.id] = data;
          iconCache = iconCache;
          // Save the cached path to the link
          await invoke("update_link", {
            id: link.id,
            name: link.name,
            url: link.url,
            icon: path,
            folder: link.folder || null,
          });
        } catch (_) {}
      }
    }
  }

  // Group links by folder
  let groupedLinks = $derived.by(() => {
    const groups = [];
    const folderMap = new Map();
    const unfiled = [];

    for (const link of links) {
      const folder = link.folder || null;
      if (folder) {
        if (!folderMap.has(folder)) {
          folderMap.set(folder, []);
        }
        folderMap.get(folder).push(link);
      } else {
        unfiled.push(link);
      }
    }

    // Folders first (sorted), then unfiled
    const sortedFolders = [...folderMap.keys()].sort((a, b) => a.localeCompare(b));
    for (const name of sortedFolders) {
      groups.push({ folder: name, links: folderMap.get(name) });
    }
    if (unfiled.length > 0) {
      groups.push({ folder: null, links: unfiled });
    }

    return groups;
  });

  let existingFolders = $derived(
    [...new Set(links.map(l => l.folder).filter(Boolean))].sort()
  );

  function toggleFolder(name) {
    const next = new Set(collapsedFolders);
    if (next.has(name)) {
      next.delete(name);
    } else {
      next.add(name);
    }
    collapsedFolders = next;
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
    formFolder = link.folder || "";
    showModal = true;
    contextMenu = null;
  }

  function resetForm() {
    formName = "";
    formUrl = "";
    formIcon = "";
    formFolder = "";
    fetchingTitle = false;
  }

  async function onUrlBlur() {
    if (!formUrl.trim() || formName.trim() || editingLink) return;
    fetchingTitle = true;
    try {
      const title = await invoke("fetch_url_metadata", { url: formUrl });
      if (!formName.trim()) {
        formName = title;
      }
    } catch (_) {}
    fetchingTitle = false;
  }

  async function saveLink() {
    const icon = formIcon || null;
    const folder = formFolder.trim() || null;
    if (editingLink) {
      await invoke("update_link", { id: editingLink.id, name: formName, url: formUrl, icon, folder });
    } else {
      await invoke("add_link", { name: formName, url: formUrl, icon, folder });
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

  // Drag-and-drop reordering
  let dragIndex = $state(null);
  let dropIndex = $state(null);
  let didDrag = false;

  function flatIndex(link) {
    return links.findIndex(l => l.id === link.id);
  }

  function onDragStart(event, link) {
    dragIndex = flatIndex(link);
    didDrag = true;
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", String(dragIndex));
  }

  function onDragOver(event, link) {
    if (dragIndex === null) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    dropIndex = flatIndex(link);
  }

  async function onDrop(event, link) {
    event.preventDefault();
    const target = flatIndex(link);
    if (dragIndex !== null && dragIndex !== target) {
      const reordered = [...links];
      const [moved] = reordered.splice(dragIndex, 1);
      reordered.splice(target, 0, moved);
      links = reordered;
      await invoke("reorder_links", { ids: reordered.map(l => l.id) });
    }
    dragIndex = null;
    dropIndex = null;
  }

  function onDragEnd() {
    dragIndex = null;
    dropIndex = null;
  }

  function handleClick(link) {
    if (didDrag) {
      didDrag = false;
      return;
    }
    openLink(link);
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
    {#each groupedLinks as group}
      {#if group.folder}
        <div class="folder-section">
          <button class="folder-header" onclick={() => toggleFolder(group.folder)}>
            <span class="folder-arrow">{collapsedFolders.has(group.folder) ? "\u25B6" : "\u25BC"}</span>
            <span class="folder-name">{group.folder}</span>
            <span class="folder-count">{group.links.length}</span>
          </button>
          {#if !collapsedFolders.has(group.folder)}
            <div class="link-grid">
              {#each group.links as link}
                {@const fi = flatIndex(link)}
                <button
                  class="link-card"
                  class:dragging={dragIndex === fi}
                  class:drop-target={dropIndex === fi && dragIndex !== fi}
                  draggable="true"
                  onclick={() => handleClick(link)}
                  oncontextmenu={(e) => showContextMenu(e, link)}
                  ondragstart={(e) => onDragStart(e, link)}
                  ondragover={(e) => onDragOver(e, link)}
                  ondrop={(e) => onDrop(e, link)}
                  ondragend={onDragEnd}
                >
                  <div class="link-icon">
                    {#if iconCache[link.id]}
                      <img src={iconCache[link.id]} alt="" onerror={(e) => e.target.style.display='none'} />
                    {/if}
                  </div>
                  <span class="link-name">{link.name}</span>
                  <span class="link-domain">{domainFromUrl(link.url)}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <div class="link-grid">
          {#each group.links as link}
            {@const fi = flatIndex(link)}
            <button
              class="link-card"
              class:dragging={dragIndex === fi}
              class:drop-target={dropIndex === fi && dragIndex !== fi}
              draggable="true"
              onclick={() => handleClick(link)}
              oncontextmenu={(e) => showContextMenu(e, link)}
              ondragstart={(e) => onDragStart(e, link)}
              ondragover={(e) => onDragOver(e, link)}
              ondrop={(e) => onDrop(e, link)}
              ondragend={onDragEnd}
            >
              <div class="link-icon">
                {#if iconCache[link.id]}
                  <img src={iconCache[link.id]} alt="" onerror={(e) => e.target.style.display='none'} />
                {/if}
              </div>
              <span class="link-name">{link.name}</span>
              <span class="link-domain">{domainFromUrl(link.url)}</span>
            </button>
          {/each}
        </div>
      {/if}
    {/each}
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
          URL
          <input type="text" bind:value={formUrl} required placeholder="https://google.com" onblur={onUrlBlur} />
        </label>

        <label>
          Name {#if fetchingTitle}<span class="hint">(fetching...)</span>{/if}
          <input type="text" bind:value={formName} required placeholder="Google" />
        </label>

        <label>
          Folder <span class="hint">(optional)</span>
          <input type="text" bind:value={formFolder} placeholder="Type or pick a folder" list="folder-list" />
          <datalist id="folder-list">
            {#each existingFolders as f}
              <option value={f} />
            {/each}
          </datalist>
        </label>

        <label>
          Icon <span class="hint">(optional — auto-fetches favicon if empty)</span>
          <input type="text" bind:value={formIcon} placeholder="/path/to/icon.png or URL" />
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
    overflow-y: auto;
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

  /* Folder sections */
  .folder-section {
    margin-bottom: 16px;
  }

  .folder-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: none;
    border-radius: 4px;
    background: none;
    cursor: pointer;
    color: var(--text);
    font-family: inherit;
    font-size: 0.9em;
    font-weight: 500;
    width: 100%;
    text-align: left;
    margin-bottom: 8px;
  }

  .folder-header:hover {
    background: var(--hover);
  }

  .folder-arrow {
    font-size: 0.7em;
    color: var(--text-muted);
    width: 12px;
  }

  .folder-name {
    flex: 1;
  }

  .folder-count {
    font-size: 0.8em;
    color: var(--text-muted);
    font-weight: 400;
  }

  .link-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 12px;
    margin-bottom: 12px;
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

  .link-card.dragging {
    opacity: 0.4;
  }

  .link-card.drop-target {
    outline: 2px dashed var(--accent);
    outline-offset: -2px;
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
