<script>
  let collapsed = $state(false);
</script>

<div class="sidebar" class:collapsed>
  <button class="collapse-toggle" onclick={() => collapsed = !collapsed} aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}>
    {collapsed ? "\u25C0" : "\u25B6"}
  </button>

  {#if !collapsed}
    <div class="sidebar-content">
      <section class="sidebar-section">
        <h3>Pinned</h3>
        <div class="placeholder">No pinned items</div>
      </section>

      <hr />

      <section class="sidebar-section">
        <h3>Status Monitor</h3>
        <div class="placeholder">No monitors configured</div>
      </section>
    </div>
  {/if}
</div>

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
</style>
