<script>
  import { invoke } from "@tauri-apps/api/core";

  let { open = $bindable(false) } = $props();

  let query = $state("");
  let items = $state([]);
  let selectedIndex = $state(0);
  let inputEl = $state(null);
  let listEl = $state(null);

  $effect(() => {
    if (open) {
      load();
    }
  });

  async function load() {
    query = "";
    selectedIndex = 0;
    try {
      const [apps, links, servers] = await Promise.all([
        invoke("get_apps"),
        invoke("get_links"),
        invoke("get_servers"),
      ]);
      items = [
        ...apps.map(a => ({
          kind: "app",
          id: a.id,
          name: a.name,
          detail: a.type_label || a.raw_command || a.executable || "",
        })),
        ...links.map(l => ({
          kind: "link",
          id: l.id,
          name: l.name,
          detail: l.url,
          url: l.url,
        })),
        ...servers.map(s => ({
          kind: "server",
          id: s.id,
          name: s.name,
          detail: s.raw_command || s.executable || "",
        })),
      ];
    } catch (e) {
      console.error("Palette load failed:", e);
      items = [];
    }
    requestAnimationFrame(() => inputEl?.focus());
  }

  let filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    // Rank: name prefix match, then name substring, then detail substring.
    const scored = [];
    for (const item of items) {
      const name = item.name.toLowerCase();
      const detail = (item.detail || "").toLowerCase();
      let score;
      if (name.startsWith(q)) score = 0;
      else if (name.includes(q)) score = 1;
      else if (detail.includes(q)) score = 2;
      else continue;
      scored.push({ item, score });
    }
    scored.sort((a, b) => a.score - b.score);
    return scored.map(s => s.item);
  });

  function onInput() {
    selectedIndex = 0;
  }

  function kindIcon(kind) {
    if (kind === "app") return "▶";
    if (kind === "link") return "\u{1F517}";
    return "⚙";
  }

  function actionLabel(kind) {
    if (kind === "app") return "Launch";
    if (kind === "link") return "Open";
    return "Start";
  }

  function scrollSelectedIntoView() {
    requestAnimationFrame(() => {
      listEl?.querySelector(".selected")?.scrollIntoView({ block: "nearest" });
    });
  }

  function onKeydown(e) {
    if (e.key === "Escape") {
      e.preventDefault();
      open = false;
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(selectedIndex + 1, filtered.length - 1);
      scrollSelectedIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(selectedIndex - 1, 0);
      scrollSelectedIntoView();
    } else if (e.key === "Enter") {
      e.preventDefault();
      activate(filtered[selectedIndex]);
    }
  }

  async function activate(item) {
    if (!item) return;
    open = false;
    try {
      if (item.kind === "app") {
        await invoke("launch_app", { id: item.id });
      } else if (item.kind === "link") {
        await invoke("open_link", { url: item.url });
      } else if (item.kind === "server") {
        await invoke("start_server", { id: item.id });
      }
    } catch (e) {
      console.error("Palette action failed:", e);
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="palette-backdrop" onclick={() => open = false} onkeydown={() => {}}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="palette" onclick={(e) => e.stopPropagation()} onkeydown={onKeydown}>
      <input
        type="text"
        class="palette-input"
        placeholder="Search apps, links, servers..."
        bind:value={query}
        bind:this={inputEl}
        oninput={onInput}
      />
      <div class="palette-list" bind:this={listEl}>
        {#if filtered.length === 0}
          <div class="palette-empty">
            {items.length === 0 ? "Nothing to search yet." : "No matches."}
          </div>
        {:else}
          {#each filtered as item, i}
            <button
              class="palette-item"
              class:selected={i === selectedIndex}
              onclick={() => activate(item)}
              onmouseenter={() => selectedIndex = i}
            >
              <span class="item-kind">{kindIcon(item.kind)}</span>
              <span class="item-name">{item.name}</span>
              <span class="item-detail">{item.detail}</span>
              <span class="item-action">{actionLabel(item.kind)}</span>
            </button>
          {/each}
        {/if}
      </div>
      <div class="palette-hint">
        <span>&uarr;&darr; navigate</span>
        <span>&crarr; select</span>
        <span>esc close</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .palette-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
    z-index: 400;
  }

  .palette {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    width: 520px;
    max-width: 90vw;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.3);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .palette-input {
    padding: 14px 16px;
    border: none;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    font-size: 1rem;
    font-family: inherit;
    outline: none;
  }

  .palette-list {
    max-height: 320px;
    overflow-y: auto;
    padding: 4px 0;
  }

  .palette-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 16px;
    border: none;
    background: none;
    cursor: pointer;
    color: var(--text);
    font-family: inherit;
    font-size: 0.9em;
    text-align: left;
  }

  .palette-item.selected {
    background: var(--hover);
  }

  .item-kind {
    flex-shrink: 0;
    font-size: 0.8em;
    width: 18px;
    text-align: center;
  }

  .item-name {
    font-weight: 500;
    flex-shrink: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item-detail {
    flex: 1;
    color: var(--text-muted);
    font-size: 0.85em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .item-action {
    flex-shrink: 0;
    font-size: 0.75em;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 6px;
    opacity: 0;
  }

  .palette-item.selected .item-action {
    opacity: 1;
  }

  .palette-empty {
    padding: 20px;
    text-align: center;
    color: var(--text-muted);
    font-size: 0.9em;
    font-style: italic;
  }

  .palette-hint {
    display: flex;
    gap: 16px;
    padding: 6px 16px;
    border-top: 1px solid var(--border);
    font-size: 0.75em;
    color: var(--text-muted);
  }
</style>
